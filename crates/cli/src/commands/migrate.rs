use std::collections::HashSet;

use futures::TryStreamExt;
use mongodb::Database;
use playlist_core::{
    generate_double_metaphone_codes, generate_n_grams,
    models::{album::Album, artist::Artist, compiler::Compiler, playlist::PlayList, track::Track},
    normalise_name, normalise_name_strong,
};

pub async fn run(database: Database) -> Result<(), Box<dyn std::error::Error>> {
    update_collection_and_field_names(database.clone()).await?;
    build_linked_tracks(database).await?;
    Ok(())
}

const JD_GROUP_ID: &str = "zmWKoBuAoSLCWDvzn";

pub async fn update_collection_and_field_names(
    database: Database,
) -> Result<(), Box<dyn std::error::Error>> {
    let compiler_docs_by_id = load_docs(database.clone(), "Compiler", mongodb::bson::doc! {})
        .await?
        .iter()
        .map(|doc| {
            let id = doc.get_str("_id").unwrap_or("").to_string();
            let new_doc = create_new_doc(doc.clone());
            let search_strings = vec![get_doc_field_or_empty_string(&new_doc, "name_normalised")];
            let new_doc = add_search_fields(new_doc, search_strings);
            (id, new_doc)
        })
        .collect::<std::collections::HashMap<String, mongodb::bson::Document>>();

    println!("Inserting compilers: {}", compiler_docs_by_id.len());
    insert_docs::<Compiler>(
        database.clone(),
        compiler_docs_by_id.values().cloned().collect(),
    )
    .await?;

    let playlist_docs = load_docs(
        database.clone(),
        "PlayList",
        mongodb::bson::doc! {"groupId": JD_GROUP_ID},
    )
    .await?
    .iter()
    .map(|doc| {
        let mut new_doc = create_new_doc(doc.clone());

        // For playlists, add search fields for all the compiler names as well as the playlist name
        let mut search_strings = vec![get_doc_field_or_empty_string(&new_doc, "name_normalised")];

        let compiler_ids = new_doc.get_array("compiler_ids").unwrap_or(&vec![]).clone();
        let compiler_names_normalised = compiler_ids
            .iter()
            .filter_map(|id_value| id_value.as_str())
            .filter_map(|id_str| compiler_docs_by_id.get(id_str))
            .map(|compiler_doc| {
                compiler_doc
                    .get_str("name_normalised")
                    .unwrap_or("")
                    .to_string()
            })
            .collect::<Vec<String>>();

        search_strings.extend(compiler_names_normalised.clone());
        new_doc = add_search_fields(new_doc, search_strings);

        new_doc
    })
    .collect::<Vec<mongodb::bson::Document>>();

    println!("Inserting playlists: {}", playlist_docs.len());
    insert_docs::<PlayList>(database.clone(), playlist_docs.clone()).await?;

    let all_playlist_track_ids = playlist_docs
        .iter()
        .flat_map(|doc| {
            doc.get_array("track_ids")
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|id_value| id_value.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .collect::<HashSet<String>>();

    // Load track docs for all tracks in playlists, finalise once artist docs are loaded (required for track search fields)
    let track_docs_partial = load_docs(
        database.clone(),
        "Track",
        mongodb::bson::doc! { "_id": { "$in": all_playlist_track_ids.iter().cloned().collect::<Vec<String>>() } },
    )
    .await?
    .iter()
    .map(|doc| {
        create_new_doc(doc.clone())
    })
    .collect::<Vec<mongodb::bson::Document>>();

    let all_artist_ids = track_docs_partial
        .iter()
        .flat_map(|doc| {
            doc.get_array("artist_ids")
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|id_value| id_value.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .collect::<HashSet<String>>();

    let artist_docs_by_id = load_docs(
        database.clone(),
        "Artist",
        mongodb::bson::doc! { "_id": { "$in": all_artist_ids.iter().cloned().collect::<Vec<String>>() } },
    )
    .await?
    .iter()
    .map(|doc| {
        let id = doc.get_str("_id").unwrap_or("").to_string();
        let new_doc = create_new_doc(doc.clone());
        let search_strings = vec![get_doc_field_or_empty_string(&new_doc, "name_normalised")];
        let new_doc = add_search_fields(new_doc, search_strings);
        (id, new_doc)
    })
    .collect::<std::collections::HashMap<String, mongodb::bson::Document>>();

    println!("Inserting artists: {}", artist_docs_by_id.len());
    insert_docs::<Artist>(
        database.clone(),
        artist_docs_by_id.values().cloned().collect(),
    )
    .await?;

    let track_docs = track_docs_partial
        .clone()
        .into_iter()
        .map(|mut doc| {
            // For tracks, add search fields for all the artist names
            let mut search_strings = vec![get_doc_field_or_empty_string(&doc, "name_normalised")];
            let artist_ids = doc.get_array("artist_ids").unwrap_or(&vec![]).clone();
            let artist_names_normalised = artist_ids
                .iter()
                .filter_map(|id_value| id_value.as_str())
                .filter_map(|id_str| artist_docs_by_id.get(id_str))
                .map(|artist_doc| {
                    artist_doc
                        .get_str("name_normalised")
                        .unwrap_or("")
                        .to_string()
                })
                .collect::<Vec<String>>();

            search_strings.extend(artist_names_normalised.clone());
            doc = add_search_fields(doc, search_strings);

            doc
        })
        .collect::<Vec<mongodb::bson::Document>>();

    println!("Inserting tracks: {}", track_docs.len());
    insert_docs::<Track>(database.clone(), track_docs).await?;

    let all_album_ids = track_docs_partial
        .iter()
        .filter_map(|doc| doc.get_str("album_id").ok().map(|s| s.to_string()))
        .collect::<HashSet<String>>();

    let album_docs = load_docs(database.clone(), "Album", mongodb::bson::doc! { "_id": { "$in": all_album_ids.iter().cloned().collect::<Vec<String>>() } })
        .await?
        .iter()
        .map(|doc| {
            let new_doc = create_new_doc(doc.clone());
            let search_strings =
                vec![get_doc_field_or_empty_string(&new_doc, "name_normalised")];
            let new_doc = add_search_fields(
                new_doc,
                search_strings,
            );
            new_doc
        })
        .collect::<Vec<mongodb::bson::Document>>();

    println!("Inserting albums: {}", album_docs.len());
    insert_docs::<Album>(database.clone(), album_docs).await?;

    Ok(())
}

struct LinkedTrackIntermediate {
    name_normalised_strong: String,
    artist_ids: HashSet<String>,
    track_ids: HashSet<String>,
}

pub async fn build_linked_tracks(database: Database) -> Result<(), Box<dyn std::error::Error>> {
    println!("Building linked tracks...");
    use playlist_core::models::server::load_music_items;
    use playlist_core::models::track::Track;

    let mut linked_tracks_by_name: std::collections::HashMap<String, Vec<LinkedTrackIntermediate>> =
        std::collections::HashMap::new();

    let tracks = load_music_items::<Track>(mongodb::bson::doc! {}).await?;
    for track in tracks {
        let name_normalised_strong = track.name_normalised_strong.clone();

        let track_artist_ids: std::collections::HashSet<String> =
            track.artist_ids.iter().cloned().collect();

        let linked_tracks = linked_tracks_by_name
            .entry(name_normalised_strong.clone())
            .or_default();

        // If an existing linked track exists for this (normalised) name with at least one of the track's artists
        let existing_linked_track = linked_tracks
            .iter_mut()
            .find(|lt| !track_artist_ids.is_disjoint(&lt.artist_ids));
        match existing_linked_track {
            Some(lt) => {
                // Add track ID to existing linked track
                lt.track_ids.insert(track.id.clone());
                // Add artist IDs to existing linked track
                for artist_id in &track.artist_ids {
                    lt.artist_ids.insert(artist_id.clone());
                }
            }
            None => {
                // Create new linked track
                linked_tracks.push(LinkedTrackIntermediate {
                    name_normalised_strong: name_normalised_strong.clone(),
                    artist_ids: track.artist_ids.iter().cloned().collect(),
                    track_ids: vec![track.id.clone()].into_iter().collect(),
                });
            }
        }
    }

    let lt_collection =
        database.collection::<playlist_core::models::track::LinkedTrack>("linked_track");
    // Remove pre-existing docs if any
    lt_collection.delete_many(mongodb::bson::doc! {}).await?;

    println!("Inserting linked tracks: {}", linked_tracks_by_name.len());

    for linked_tracks in linked_tracks_by_name.values() {
        for lt in linked_tracks {
            if lt.track_ids.len() < 2 {
                continue;
            }
            let new_lt = playlist_core::models::track::LinkedTrack {
                id: playlist_core::database::generate_id(),
                track_name_normalised_strong: lt.name_normalised_strong.clone(),
                track_ids: lt.track_ids.iter().cloned().collect(),
                artist_ids: lt.artist_ids.iter().cloned().collect(),
            };

            lt_collection.insert_one(&new_lt).await?;
        }
    }

    Ok(())
}

async fn load_docs(
    database: Database,
    collection_name: &str,
    query: mongodb::bson::Document,
) -> Result<Vec<mongodb::bson::Document>, Box<dyn std::error::Error>> {
    let collection = database.collection::<mongodb::bson::Document>(collection_name);
    let docs = collection
        .find(query)
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    Ok(docs)
}

fn create_new_doc(original_doc: mongodb::bson::Document) -> mongodb::bson::Document {
    static FIELDS_TO_REMOVE: [&str; 7] = [
        "appearsInPlayLists",
        "appearsInPlayListGroups",
        "name_normalised",
        "name_normalised_strong",
        "spotify_id",
        "spotify_user_id",
        "mb_id",
    ];

    let mut new_doc = mongodb::bson::Document::new();

    for (key, value) in original_doc.iter() {
        if !FIELDS_TO_REMOVE.contains(&key.as_str()) {
            let new_key = camel_to_snake_case(key);
            new_doc.insert(new_key, value.clone());
        }
    }

    // Add/update normalised name fields using current implementations
    let name_normalised = normalise_name(original_doc.get_str("name").unwrap_or(""));
    new_doc.insert("name_normalised", name_normalised.clone());
    new_doc.insert(
        "name_normalised_strong",
        normalise_name_strong(original_doc.get_str("name").unwrap_or("")),
    );

    // Add external service associations using new structure
    let mut external_service_associations: Vec<mongodb::bson::Document> = vec![];
    if let Some(spotify_id) = original_doc.get_str("spotifyId").ok() {
        let image_urls = original_doc.get_document("imageURLs").ok().and_then(|doc| {
            let small = doc.get_str("small").ok().map(|s| s.to_string());
            let medium = doc.get_str("medium").ok().map(|s| s.to_string());
            let large = doc.get_str("large").ok().map(|s| s.to_string());
            if small.is_none() && medium.is_none() && large.is_none() {
                None
            } else {
                Some(mongodb::bson::doc! {
                    "small": small,
                    "medium": medium,
                    "large": large,
                })
            }
        });
        let association_doc = mongodb::bson::doc! {
            "Spotify": {
                "id": spotify_id.to_string(),
                "image_urls": image_urls,
            }
        };
        external_service_associations.push(association_doc);
    }
    if let Some(mb_id) = original_doc.get_str("mbId").ok() {
        external_service_associations.push(mongodb::bson::doc! {
            "MusicBrainz": {
                "id": mb_id.to_string(),
            }
        });
    }
    if !external_service_associations.is_empty() {
        new_doc.insert(
            "external_service_associations",
            external_service_associations,
        );
    }

    new_doc
}

fn add_search_fields(
    mut doc: mongodb::bson::Document,
    normalised_search_strings: Vec<String>,
) -> mongodb::bson::Document {
    // Add/update search fields
    let mut search_terms = HashSet::<String>::new();
    let mut double_metaphone_codes = HashSet::<String>::new();
    let mut n_grams = HashSet::<String>::new();

    for search_string in &normalised_search_strings {
        for term in search_string.split_whitespace() {
            search_terms.insert(term.to_string());
            double_metaphone_codes.extend(generate_double_metaphone_codes(term));
            n_grams.extend(generate_n_grams(term));
        }
    }

    doc.insert(
        "search_terms",
        search_terms.into_iter().collect::<Vec<String>>(),
    );
    doc.insert(
        "search_double_metaphone_codes",
        double_metaphone_codes.into_iter().collect::<Vec<String>>(),
    );
    doc.insert(
        "search_n_grams",
        n_grams.into_iter().collect::<Vec<String>>(),
    );

    doc
}

async fn insert_docs<T>(
    database: Database,
    // collection_name: &str,
    docs: Vec<mongodb::bson::Document>,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: playlist_core::models::MusicItem
        + serde::de::DeserializeOwned
        + serde::Serialize
        + Send
        + Sync,
{
    let typed_docs = docs
        .into_iter()
        .map(|doc| mongodb::bson::from_document::<T>(doc))
        .collect::<Result<Vec<_>, _>>()?;

    let collection = database.collection::<T>(T::collection_name());
    collection.delete_many(mongodb::bson::doc! {}).await?;
    collection.insert_many(typed_docs).await?;
    Ok(())
}

fn get_doc_field_or_empty_string(doc: &mongodb::bson::Document, field_name: &str) -> String {
    doc.get_str(field_name).unwrap_or("").to_string()
}

fn camel_to_snake_case(text: &str) -> String {
    // Special cases
    let text = text.replace("PlayList", "Playlist").replace("URL", "Url");

    let mut buffer = String::with_capacity(text.len() + text.len() / 2);

    let mut text = text.chars();

    if let Some(first) = text.next() {
        let mut n2: Option<(bool, char)> = None;
        let mut n1: (bool, char) = (first.is_lowercase(), first);

        for c in text {
            let prev_n1 = n1.clone();

            let n3 = n2;
            n2 = Some(n1);
            n1 = (c.is_lowercase(), c);

            // insert underscore if acronym at beginning
            // ABc -> a_bc
            if let Some((false, c3)) = n3
                && let Some((false, c2)) = n2
                && n1.0
                && c3.is_uppercase()
                && c2.is_uppercase()
            {
                buffer.push('_');
            }

            buffer.push_str(&prev_n1.1.to_lowercase().to_string());

            // insert underscore before next word
            // abC -> ab_c
            if let Some((true, _)) = n2
                && n1.1.is_uppercase()
            {
                buffer.push('_');
            }
        }

        buffer.push_str(&n1.1.to_lowercase().to_string());
    }

    buffer
}
