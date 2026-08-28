//! Merge two records of the same type into one.
//!
//! When the same real-world entity is imported from more than one music service — for
//! example the same person compiling a session, imported once from a Spotify playlist and
//! once from the equivalent Tidal playlist — the importer creates a separate record for
//! each service, because it deduplicates on a service-specific external id (see
//! `import_playlist::find_existing_id`). This command folds one such record (the "remove"
//! record) into another (the "keep" record):
//!
//! 1. The kept record gains the removed record's external service associations, so a single
//!    record ends up carrying both services' ids and images.
//! 2. Every reference to the removed record elsewhere in the database is repointed to the
//!    kept record.
//! 3. The removed record is deleted.
//!
//! It works for every music item type — artist, album, track, compiler and playlist — since
//! the cross-service duplication it resolves applies to all of them. A `--dry-run` reports
//! exactly what would change without writing anything.
//!
//! The kept record's own descriptive fields win: its name and search index are the ones that
//! survive, and its `disambiguation` and `notes` are kept, filled from the removed record only
//! where the kept record has none. Two exceptions carry information forward rather than losing
//! it with the deleted record: a removed artist whose name differs from the kept artist's is
//! preserved as an `alt_names` entry, and the kept artist's search index is rebuilt to include
//! those alternates so the merged artist stays findable by either spelling; and a `needs_review`
//! flag on the removed record is propagated to the kept record.

use crate::commands::import_playlist::build_search_fields;
use futures::stream::TryStreamExt;
use mongodb::Database;
use mongodb::bson::{Bson, Document, doc};
use playlist_core::models::{
    MusicItemBase, album::Album, artist::Artist, compiler::Compiler, playlist::PlayList,
    track::Track,
};
use playlist_core::normalise_name;
use std::collections::HashSet;

/// The collection holding the derived track-linking documents. It is not a music item, so
/// it has no model constant; the name is defined where the collection is read and written
/// (see `playlist_core::models::track::load_linked_tracks` and
/// `commands::migrate::build_linked_tracks`).
const LINKED_TRACK_COLLECTION: &str = "linked_track";

/// A music item type that can be merged, naming the collection it lives in and the places
/// other records point at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordType {
    Artist,
    Album,
    Track,
    Compiler,
    Playlist,
}

/// A place one collection references records of the type being merged, so a merge can
/// repoint it from the removed id to the kept id.
struct Reference {
    /// The collection whose documents hold the reference.
    collection: &'static str,
    /// The field within those documents holding the reference.
    field: &'static str,
    /// Whether the field is an array of ids or a single id.
    kind: ReferenceKind,
}

#[derive(Clone, Copy)]
enum ReferenceKind {
    /// An array of ids (for example `track.artist_ids`).
    Array,
    /// A single id (for example `track.album_id`).
    Scalar,
}

impl Reference {
    fn array(collection: &'static str, field: &'static str) -> Self {
        Reference {
            collection,
            field,
            kind: ReferenceKind::Array,
        }
    }

    fn scalar(collection: &'static str, field: &'static str) -> Self {
        Reference {
            collection,
            field,
            kind: ReferenceKind::Scalar,
        }
    }
}

impl RecordType {
    /// Parse the command-line type argument.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "artist" => Some(RecordType::Artist),
            "album" => Some(RecordType::Album),
            "track" => Some(RecordType::Track),
            "compiler" => Some(RecordType::Compiler),
            "playlist" => Some(RecordType::Playlist),
            _ => None,
        }
    }

    /// The list of accepted type arguments, for usage messages.
    pub const VALID: &'static str = "artist, album, track, compiler, playlist";

    /// The MongoDB collection this type's records live in.
    fn collection(self) -> &'static str {
        match self {
            RecordType::Artist => <Artist as MusicItemBase>::collection_name(),
            RecordType::Album => <Album as MusicItemBase>::collection_name(),
            RecordType::Track => <Track as MusicItemBase>::collection_name(),
            RecordType::Compiler => <Compiler as MusicItemBase>::collection_name(),
            RecordType::Playlist => <PlayList as MusicItemBase>::collection_name(),
        }
    }

    /// Everywhere a record of this type is referenced by other records.
    ///
    /// A playlist is referenced by nothing (its `group_id` and `tag_ids` point at groups and
    /// tags, not at other playlists), so merging playlists only unifies associations and
    /// deletes the removed record.
    fn inbound_references(self) -> Vec<Reference> {
        let track = <Track as MusicItemBase>::collection_name();
        let album = <Album as MusicItemBase>::collection_name();
        let playlist = <PlayList as MusicItemBase>::collection_name();
        match self {
            RecordType::Artist => vec![
                Reference::array(track, "artist_ids"),
                Reference::array(album, "artist_ids"),
                Reference::array(LINKED_TRACK_COLLECTION, "artist_ids"),
            ],
            RecordType::Album => vec![Reference::scalar(track, "album_id")],
            RecordType::Track => vec![
                Reference::array(playlist, "track_ids"),
                Reference::array(LINKED_TRACK_COLLECTION, "track_ids"),
            ],
            RecordType::Compiler => vec![Reference::array(playlist, "compiler_ids")],
            RecordType::Playlist => vec![],
        }
    }
}

/// Merge the `remove_id` record of type `type_str` into the `keep_id` record: unify their
/// external service associations onto the kept record, repoint every reference to the removed
/// record, and delete it. With `dry_run` set, report what would change and write nothing.
pub async fn merge_records(
    database: Database,
    type_str: &str,
    keep_id: &str,
    remove_id: &str,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let record_type = RecordType::parse(type_str).ok_or_else(|| {
        format!(
            "Unknown record type \"{type_str}\". Valid types: {}.",
            RecordType::VALID
        )
    })?;

    if keep_id == remove_id {
        return Err("The keep and remove ids must be different.".into());
    }

    let collection_name = record_type.collection();
    let collection = database.collection::<Document>(collection_name);

    let keep_doc = collection
        .find_one(doc! { "_id": keep_id })
        .await?
        .ok_or_else(|| {
            format!("No {type_str} found with id \"{keep_id}\" (the record to keep).")
        })?;
    let remove_doc = collection
        .find_one(doc! { "_id": remove_id })
        .await?
        .ok_or_else(|| {
            format!("No {type_str} found with id \"{remove_id}\" (the record to remove).")
        })?;

    let prefix = if dry_run { "[dry-run] " } else { "" };
    println!("{prefix}Merging {type_str} \"{remove_id}\" into \"{keep_id}\".");

    // 1. Fold the removed record's common fields onto the kept record.
    let mut set = Document::new();

    let keep_assocs = keep_doc.get_array("external_service_associations").ok();
    let remove_assocs = remove_doc.get_array("external_service_associations").ok();
    let merged_assocs = merge_association_lists(keep_assocs, remove_assocs);
    println!(
        "  external service associations: {} -> {}",
        keep_assocs.map(Vec::len).unwrap_or(0),
        merged_assocs.len()
    );
    // Only write the field when there is something to write, so two records that both lack
    // associations are not given an empty array where they previously had no field at all.
    if !merged_assocs.is_empty() {
        set.insert("external_service_associations", Bson::Array(merged_assocs));
    }

    // Fill disambiguation and notes from the removed record only where the kept record has
    // none, so no information the kept record already carries is overwritten.
    for field in ["disambiguation", "notes"] {
        if keep_doc.get_str(field).is_ok() {
            continue;
        }
        if let Ok(value) = remove_doc.get_str(field) {
            set.insert(field, value);
        }
    }

    // Carry a review flag forward: if the removed record was flagged for review, the merged
    // record should be too, so the signal is not lost when the removed record is deleted.
    if remove_doc.get_bool("needs_review").unwrap_or(false)
        && !keep_doc.get_bool("needs_review").unwrap_or(false)
    {
        set.insert("needs_review", true);
    }

    // Preserve the removed record's spelling of the name as an alternate (artists only carry
    // alt_names), so a name that differs between services is not lost, and rebuild the kept
    // artist's search index to include those alternates so it stays findable by either spelling.
    if record_type == RecordType::Artist {
        let alt_names = merge_alt_names(&keep_doc, &remove_doc);
        if !alt_names.is_empty() {
            let mut search_strings = vec![normalise_name(keep_doc.get_str("name").unwrap_or(""))];
            search_strings.extend(alt_names.iter().map(|name| normalise_name(name)));
            let (search_terms, double_metaphone_codes, n_grams) =
                build_search_fields(&search_strings);
            set.insert("search_terms", search_terms);
            set.insert("search_double_metaphone_codes", double_metaphone_codes);
            set.insert("search_n_grams", n_grams);
            set.insert(
                "alt_names",
                Bson::Array(alt_names.into_iter().map(Bson::String).collect()),
            );
        }
    }

    // Skip the update when nothing on the kept record changes: MongoDB rejects an empty
    // `$set`, and there is nothing to write anyway (for example merging two records that both
    // carry no associations and nothing to fill).
    if !dry_run && !set.is_empty() {
        collection
            .update_one(doc! { "_id": keep_id }, doc! { "$set": set })
            .await?;
    }

    // 2. Repoint every reference to the removed record.
    for reference in record_type.inbound_references() {
        let updated = match reference.kind {
            ReferenceKind::Array => {
                repoint_array_references(
                    &database,
                    reference.collection,
                    reference.field,
                    remove_id,
                    keep_id,
                    dry_run,
                )
                .await?
            }
            ReferenceKind::Scalar => {
                repoint_scalar_references(
                    &database,
                    reference.collection,
                    reference.field,
                    remove_id,
                    keep_id,
                    dry_run,
                )
                .await?
            }
        };
        println!(
            "  {}.{}: repointed {updated} document(s).",
            reference.collection, reference.field
        );
    }

    // 3. Delete the removed record.
    if !dry_run {
        collection.delete_one(doc! { "_id": remove_id }).await?;
    }
    println!("{prefix}Deleted {type_str} \"{remove_id}\".");
    println!("{prefix}Merge complete.");

    Ok(())
}

/// Replace `remove_id` with `keep_id` in an id list, keeping `keep_id` at a single entry in
/// the first position it occupies. These reference lists name each entity at most once, so once
/// `keep_id` and `remove_id` are the same entity it should appear once; every duplicate
/// `keep_id` — one the replacement creates and any that was already present — collapses into
/// that first entry. Every other id is left exactly as found, including a repeated unrelated id,
/// so the merge changes only the merged entity's entries. In practice this function only ever
/// sees lists that already contain `remove_id`, since it is applied to the documents a merge
/// selects by that id.
pub(crate) fn merge_id_in_list(list: &[String], remove_id: &str, keep_id: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(list.len());
    for item in list {
        let mapped = if item == remove_id {
            keep_id
        } else {
            item.as_str()
        };
        if mapped == keep_id && result.iter().any(|existing| existing == keep_id) {
            continue;
        }
        result.push(mapped.to_string());
    }
    result
}

/// Union the removed record's external service associations into the kept record's,
/// preserving the kept record's order and its version of any association they share, and
/// appending each removed association whose `(service, id)` the kept record does not already
/// carry. Associations that do not parse as a `(service, id)` pair are appended unchanged.
fn merge_association_lists(keep: Option<&Vec<Bson>>, remove: Option<&Vec<Bson>>) -> Vec<Bson> {
    let mut result: Vec<Bson> = keep.cloned().unwrap_or_default();
    let mut seen: HashSet<(String, String)> = result.iter().filter_map(association_key).collect();
    if let Some(list) = remove {
        for association in list {
            match association_key(association) {
                Some(key) => {
                    if seen.insert(key) {
                        result.push(association.clone());
                    }
                }
                None => result.push(association.clone()),
            }
        }
    }
    result
}

/// The `(service, id)` identity of an external service association, used to deduplicate a
/// merged association list. Associations are stored externally tagged — a single-key document
/// such as `{ "Spotify": { "id": "abc", ... } }` — so the sole key names the service and the
/// nested `id` names the record on it.
fn association_key(association: &Bson) -> Option<(String, String)> {
    let document = association.as_document()?;
    let (service, value) = document.iter().next()?;
    let id = value.as_document()?.get_str("id").ok()?;
    Some((service.clone(), id.to_string()))
}

/// Build the kept artist's `alt_names`: its existing alternates first, then the removed
/// artist's name, then the removed artist's own alternates. Entries are deduplicated by their
/// normalised form and the kept artist's own name is excluded, while the original spelling of
/// each retained alternate is kept.
fn merge_alt_names(keep: &Document, remove: &Document) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Never list the kept record's own name as one of its alternates.
    seen.insert(normalise_name(keep.get_str("name").unwrap_or("")));

    let push_unique = |candidate: &str, result: &mut Vec<String>, seen: &mut HashSet<String>| {
        let normalised = normalise_name(candidate);
        if !normalised.is_empty() && seen.insert(normalised) {
            result.push(candidate.to_string());
        }
    };

    if let Ok(existing) = keep.get_array("alt_names") {
        for value in existing {
            if let Some(name) = value.as_str() {
                push_unique(name, &mut result, &mut seen);
            }
        }
    }
    if let Ok(name) = remove.get_str("name") {
        push_unique(name, &mut result, &mut seen);
    }
    if let Ok(existing) = remove.get_array("alt_names") {
        for value in existing {
            if let Some(name) = value.as_str() {
                push_unique(name, &mut result, &mut seen);
            }
        }
    }

    result
}

/// Repoint an array-of-ids reference field from `remove_id` to `keep_id` across a collection,
/// returning the number of documents that referenced the removed record. Each affected
/// document's array is rewritten with [`merge_id_in_list`].
async fn repoint_array_references(
    database: &Database,
    collection_name: &str,
    field: &str,
    remove_id: &str,
    keep_id: &str,
    dry_run: bool,
) -> Result<u64, Box<dyn std::error::Error>> {
    let collection = database.collection::<Document>(collection_name);
    // Collect the matches before writing so no cursor is open over the documents being updated
    // (an update that removes the matched id would otherwise disturb an in-flight query).
    let affected: Vec<Document> = collection
        .find(doc! { field: remove_id })
        .await?
        .try_collect()
        .await?;

    for document in &affected {
        let Ok(id) = document.get_str("_id") else {
            continue;
        };
        let current: Vec<String> = document
            .get_array(field)
            .map(|array| {
                array
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let updated = merge_id_in_list(&current, remove_id, keep_id);
        if !dry_run {
            let updated: Vec<Bson> = updated.into_iter().map(Bson::String).collect();
            collection
                .update_one(doc! { "_id": id }, doc! { "$set": { field: updated } })
                .await?;
        }
    }

    Ok(affected.len() as u64)
}

/// Repoint a single-id reference field from `remove_id` to `keep_id` across a collection,
/// returning the number of documents that referenced the removed record.
async fn repoint_scalar_references(
    database: &Database,
    collection_name: &str,
    field: &str,
    remove_id: &str,
    keep_id: &str,
    dry_run: bool,
) -> Result<u64, Box<dyn std::error::Error>> {
    let collection = database.collection::<Document>(collection_name);
    let count = collection
        .count_documents(doc! { field: remove_id })
        .await?;
    if !dry_run && count > 0 {
        collection
            .update_many(
                doc! { field: remove_id },
                doc! { "$set": { field: keep_id } },
            )
            .await?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::bson;

    // --- merge_id_in_list ---

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn merge_id_in_list_replaces_the_removed_id_in_place() {
        assert_eq!(
            merge_id_in_list(&ids(&["a", "remove", "b"]), "remove", "keep"),
            ids(&["a", "keep", "b"])
        );
    }

    #[test]
    fn merge_id_in_list_preserves_order_and_other_ids() {
        assert_eq!(
            merge_id_in_list(&ids(&["x", "y", "remove", "z"]), "remove", "keep"),
            ids(&["x", "y", "keep", "z"])
        );
    }

    #[test]
    fn merge_id_in_list_collapses_keep_and_remove_appearing_together() {
        // The list already had keep; replacing remove with keep would list keep twice, so the
        // duplicate is collapsed to the first position keep held.
        assert_eq!(
            merge_id_in_list(&ids(&["keep", "a", "remove"]), "remove", "keep"),
            ids(&["keep", "a"])
        );
        assert_eq!(
            merge_id_in_list(&ids(&["remove", "a", "keep"]), "remove", "keep"),
            ids(&["keep", "a"])
        );
    }

    #[test]
    fn merge_id_in_list_leaves_unrelated_repeats_untouched() {
        // A genuinely repeated unrelated id is not deduplicated — only the merged entity's own
        // entries collapse.
        assert_eq!(
            merge_id_in_list(&ids(&["a", "a", "remove", "b"]), "remove", "keep"),
            ids(&["a", "a", "keep", "b"])
        );
    }

    #[test]
    fn merge_id_in_list_collapses_a_pre_existing_keep_duplicate() {
        // These reference lists name each entity once, so keep ends up at a single entry even
        // when it was already duplicated in a list that also held remove.
        assert_eq!(
            merge_id_in_list(&ids(&["keep", "keep", "remove"]), "remove", "keep"),
            ids(&["keep"])
        );
    }

    #[test]
    fn merge_id_in_list_without_the_removed_id_is_unchanged() {
        assert_eq!(
            merge_id_in_list(&ids(&["a", "b", "c"]), "remove", "keep"),
            ids(&["a", "b", "c"])
        );
    }

    #[test]
    fn merge_id_in_list_collapses_repeated_removed_ids_into_one_keep() {
        assert_eq!(
            merge_id_in_list(&ids(&["remove", "a", "remove"]), "remove", "keep"),
            ids(&["keep", "a"])
        );
    }

    // --- association_key ---

    #[test]
    fn association_key_reads_service_and_id() {
        let spotify = bson!({ "Spotify": { "id": "sp1", "image_urls": { "small": "s" } } });
        assert_eq!(
            association_key(&spotify),
            Some(("Spotify".to_string(), "sp1".to_string()))
        );
        let musicbrainz = bson!({ "MusicBrainz": { "id": "mb1" } });
        assert_eq!(
            association_key(&musicbrainz),
            Some(("MusicBrainz".to_string(), "mb1".to_string()))
        );
    }

    #[test]
    fn association_key_is_none_for_unexpected_shapes() {
        assert_eq!(association_key(&bson!("not a document")), None);
        assert_eq!(
            association_key(&bson!({ "Spotify": { "no_id": "x" } })),
            None
        );
    }

    // --- merge_association_lists ---

    fn keys(associations: &[Bson]) -> Vec<(String, String)> {
        associations.iter().filter_map(association_key).collect()
    }

    #[test]
    fn merge_association_lists_unions_across_services() {
        let keep = vec![bson!({ "Spotify": { "id": "sp1" } })];
        let remove = vec![bson!({ "Tidal": { "id": "td1" } })];
        let merged = merge_association_lists(Some(&keep), Some(&remove));
        assert_eq!(
            keys(&merged),
            vec![
                ("Spotify".to_string(), "sp1".to_string()),
                ("Tidal".to_string(), "td1".to_string()),
            ]
        );
    }

    #[test]
    fn merge_association_lists_keeps_the_kept_records_version_of_a_shared_association() {
        // Same service and id on both sides: the kept record's copy (with images) survives and
        // the removed record's copy is not appended.
        let keep = vec![bson!({ "Spotify": { "id": "sp1", "image_urls": { "small": "keep" } } })];
        let remove = vec![bson!({ "Spotify": { "id": "sp1", "image_urls": { "small": "gone" } } })];
        let merged = merge_association_lists(Some(&keep), Some(&remove));
        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0]
                .as_document()
                .unwrap()
                .get_document("Spotify")
                .unwrap()
                .get_document("image_urls")
                .unwrap()
                .get_str("small")
                .unwrap(),
            "keep"
        );
    }

    #[test]
    fn merge_association_lists_keeps_two_records_from_the_same_service_with_different_ids() {
        let keep = vec![bson!({ "Spotify": { "id": "sp1" } })];
        let remove = vec![bson!({ "Spotify": { "id": "sp2" } })];
        let merged = merge_association_lists(Some(&keep), Some(&remove));
        assert_eq!(
            keys(&merged),
            vec![
                ("Spotify".to_string(), "sp1".to_string()),
                ("Spotify".to_string(), "sp2".to_string()),
            ]
        );
    }

    #[test]
    fn merge_association_lists_handles_missing_sides() {
        let remove = vec![bson!({ "Tidal": { "id": "td1" } })];
        assert_eq!(
            keys(&merge_association_lists(None, Some(&remove))),
            vec![("Tidal".to_string(), "td1".to_string())]
        );
        let keep = vec![bson!({ "Spotify": { "id": "sp1" } })];
        assert_eq!(
            keys(&merge_association_lists(Some(&keep), None)),
            vec![("Spotify".to_string(), "sp1".to_string())]
        );
        assert!(merge_association_lists(None, None).is_empty());
    }

    // --- merge_alt_names ---

    fn doc_with(name: &str, alt_names: Option<Vec<&str>>) -> Document {
        let mut document = doc! { "name": name };
        if let Some(alt) = alt_names {
            document.insert(
                "alt_names",
                alt.into_iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            );
        }
        document
    }

    #[test]
    fn merge_alt_names_records_the_removed_name_when_it_differs() {
        let keep = doc_with("The Beatles", None);
        let remove = doc_with("Beatles", None);
        assert_eq!(merge_alt_names(&keep, &remove), vec!["Beatles".to_string()]);
    }

    #[test]
    fn merge_alt_names_skips_the_removed_name_when_it_matches_after_normalising() {
        // Case and accents fall away under normalisation, so "BEYONCÉ" is recognised as the
        // kept record's own name ("Beyoncé") and not recorded as an alternate.
        let keep = doc_with("Beyoncé", None);
        let remove = doc_with("BEYONCÉ", None);
        assert!(merge_alt_names(&keep, &remove).is_empty());
    }

    #[test]
    fn merge_alt_names_unions_existing_alternates_from_both_records() {
        let keep = doc_with("A Name", Some(vec!["First Alt"]));
        let remove = doc_with("Other Name", Some(vec!["Second Alt", "First Alt"]));
        assert_eq!(
            merge_alt_names(&keep, &remove),
            vec![
                "First Alt".to_string(),
                "Other Name".to_string(),
                "Second Alt".to_string(),
            ]
        );
    }

    // --- RecordType ---

    #[test]
    fn record_type_parses_known_types_only() {
        assert_eq!(RecordType::parse("artist"), Some(RecordType::Artist));
        assert_eq!(RecordType::parse("album"), Some(RecordType::Album));
        assert_eq!(RecordType::parse("track"), Some(RecordType::Track));
        assert_eq!(RecordType::parse("compiler"), Some(RecordType::Compiler));
        assert_eq!(RecordType::parse("playlist"), Some(RecordType::Playlist));
        assert_eq!(RecordType::parse("Artist"), None);
        assert_eq!(RecordType::parse("group"), None);
        assert_eq!(RecordType::parse(""), None);
    }

    #[test]
    fn record_type_collections_match_the_models() {
        assert_eq!(RecordType::Artist.collection(), "artist");
        assert_eq!(RecordType::Album.collection(), "album");
        assert_eq!(RecordType::Track.collection(), "track");
        assert_eq!(RecordType::Compiler.collection(), "compiler");
        assert_eq!(RecordType::Playlist.collection(), "playlist");
    }

    #[test]
    fn playlist_has_no_inbound_references() {
        assert!(RecordType::Playlist.inbound_references().is_empty());
    }

    #[test]
    fn artist_is_referenced_by_tracks_albums_and_linked_tracks() {
        let references: Vec<(&str, &str)> = RecordType::Artist
            .inbound_references()
            .iter()
            .map(|reference| (reference.collection, reference.field))
            .collect();
        assert_eq!(
            references,
            vec![
                ("track", "artist_ids"),
                ("album", "artist_ids"),
                ("linked_track", "artist_ids"),
            ]
        );
    }
}
