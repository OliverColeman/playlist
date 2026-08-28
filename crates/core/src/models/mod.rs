use serde::{Deserialize, Serialize};

pub mod album;
pub mod artist;
pub mod compiler;
pub mod playlist;
pub mod track;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemType {
    Track,
    Artist,
    Album,
    Playlist,
    Compiler,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExternalServiceAssociation {
    Spotify {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        image_urls: Option<ImageUrls>,
    },
    Tidal {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        image_urls: Option<ImageUrls>,
    },
    MusicBrainz {
        id: String,
    },
}

/// Trait for all music items with common fields
pub trait MusicItemBase {
    fn item_type() -> ItemType;
    fn collection_name() -> &'static str;
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn name_normalised(&self) -> &str;
    fn name_normalised_strong(&self) -> &str;
    fn disambiguation(&self) -> Option<&str>;
    fn notes(&self) -> Option<&str>;
    fn data_maybe_missing(&self) -> Option<&[String]>;
    fn potential_duplicate(&self) -> Option<bool>;
    fn needs_review(&self) -> Option<bool>;
    fn search_terms(&self) -> &[String];
    fn search_double_metaphone_codes(&self) -> &[String];
    fn search_n_grams(&self) -> &[String];
    fn external_service_associations(&self) -> Option<&[ExternalServiceAssociation]>;
}

pub trait MusicItem: MusicItemBase + Clone {}

#[macro_export]
macro_rules! define_music_item_struct_with_common_fields {
    (
        $name:ident,
        $item_type:expr,
        $collection_name:expr,
        { $($(#[$attr:meta])* $field_name:ident : $field_type:ty),* $(,)? }
    ) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct $name {
            #[serde(rename = "_id")]
            pub id: String,

            pub name: String,

            /// Normalized version of the name (from normaliseString)
            pub name_normalised: String,

            /// Used for LinkedTrack matching (from normaliseStringStrong)
            pub name_normalised_strong: String,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub disambiguation: Option<String>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub notes: Option<String>,

            /// The name of the field that may be missing data
            #[serde(skip_serializing_if = "Option::is_none")]
            pub data_maybe_missing: Option<Vec<String>>,

            /// This isn't actually set anywhere, still required?
            #[serde(skip_serializing_if = "Option::is_none")]
            pub potential_duplicate: Option<bool>,

            /// This is used to manually indicate that the item needs review.
            /// An item also "needs review" if the dataMaybeMissing field is populated,
            /// and probably if it is not linked to a music service,
            /// and if there appear to be duplicates.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub needs_review: Option<bool>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub external_service_associations: Option<Vec<crate::models::ExternalServiceAssociation>>,

            pub search_terms: Vec<String>,
            pub search_double_metaphone_codes: Vec<String>,
            pub search_n_grams: Vec<String>,

            $($(#[$attr])* pub $field_name: $field_type,)*
        }

        impl crate::models::MusicItemBase for $name {
            fn item_type() -> crate::models::ItemType { $item_type }
            fn collection_name() -> &'static str { $collection_name }
            fn id(&self) -> &str { &self.id }
            fn name(&self) -> &str { &self.name }
            fn name_normalised(&self) -> &str { &self.name_normalised }
            fn name_normalised_strong(&self) -> &str { &self.name_normalised_strong }
            fn disambiguation(&self) -> Option<&str> { self.disambiguation.as_deref() }
            fn notes(&self) -> Option<&str> { self.notes.as_deref() }
            fn data_maybe_missing(&self) -> Option<&[String]> { self.data_maybe_missing.as_deref() }
            fn potential_duplicate(&self) -> Option<bool> { self.potential_duplicate }
            fn needs_review(&self) -> Option<bool> { self.needs_review }
            fn external_service_associations(&self) -> Option<&[crate::models::ExternalServiceAssociation]> { self.external_service_associations.as_deref() }
            fn search_double_metaphone_codes(&self) -> &[String] { &self.search_double_metaphone_codes }
            fn search_n_grams(&self) -> &[String] { &self.search_n_grams }
            fn search_terms(&self) -> &[String] { &self.search_terms }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }

        impl Unpin for $name {}
        impl crate::models::MusicItem for $name {}
    };
}

pub trait MusicItemCollection<T: crate::models::MusicItem> {
    fn sorted_by_name_normalised(&self, direction: i8) -> Vec<T>;
    fn get(&self, id: &str) -> Option<&T>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MusicItemsById<T: MusicItem> {
    pub by_id: std::collections::HashMap<String, T>,
}

impl<T: MusicItem> MusicItemCollection<T> for MusicItemsById<T> {
    fn sorted_by_name_normalised(&self, direction: i8) -> Vec<T> {
        let mut items: Vec<T> = self.by_id.values().cloned().collect();
        items.sort_by(|a, b| {
            if direction >= 0 {
                a.name_normalised().cmp(b.name_normalised())
            } else {
                b.name_normalised().cmp(a.name_normalised())
            }
        });
        items
    }

    fn get(&self, id: &str) -> Option<&T> {
        self.by_id.get(id)
    }
}

impl<T: MusicItem> From<Vec<T>> for MusicItemsById<T> {
    fn from(items: Vec<T>) -> Self {
        let mut by_id = std::collections::HashMap::new();
        for item in items {
            by_id.insert(item.id().to_string(), item);
        }
        MusicItemsById { by_id }
    }
}

impl<T: MusicItem> From<MusicItemsById<T>> for Vec<T>
where
    T: crate::models::MusicItem,
{
    fn from(music_items_by_id: MusicItemsById<T>) -> Self {
        music_items_by_id.by_id.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::compiler::Compiler;
    use crate::models::playlist::PlayList;
    use crate::models::track::Track;

    fn make_compiler(id: &str, name: &str) -> Compiler {
        Compiler {
            id: id.to_string(),
            name: name.to_string(),
            name_normalised: crate::normalise_name(name),
            name_normalised_strong: crate::normalise_name_strong(name),
            disambiguation: None,
            notes: None,
            data_maybe_missing: None,
            potential_duplicate: None,
            needs_review: None,
            external_service_associations: None,
            search_terms: vec![name.to_lowercase()],
            search_double_metaphone_codes: vec![],
            search_n_grams: vec![],
        }
    }

    fn make_track(id: &str, name: &str) -> Track {
        Track {
            id: id.to_string(),
            name: name.to_string(),
            name_normalised: crate::normalise_name(name),
            name_normalised_strong: crate::normalise_name_strong(name),
            disambiguation: None,
            notes: None,
            data_maybe_missing: None,
            potential_duplicate: None,
            needs_review: None,
            external_service_associations: None,
            search_terms: vec![name.to_lowercase()],
            search_double_metaphone_codes: vec![],
            search_n_grams: vec![],
            artist_ids: vec!["artist1".to_string()],
            album_id: None,
            duration: Some(215.5),
        }
    }

    #[test]
    fn music_items_by_id_from_vec_keys_items_by_id() {
        let collection = MusicItemsById::from(vec![
            make_compiler("c1", "Alpha"),
            make_compiler("c2", "Beta"),
        ]);
        assert_eq!(collection.by_id.len(), 2);
        assert_eq!(collection.by_id["c1"].name, "Alpha");
        assert_eq!(collection.by_id["c2"].name, "Beta");
    }

    #[test]
    fn music_items_by_id_from_vec_last_item_wins_on_duplicate_ids() {
        // Documents current behavior: later items silently replace earlier ones.
        let collection = MusicItemsById::from(vec![
            make_compiler("c1", "Original"),
            make_compiler("c1", "Replacement"),
        ]);
        assert_eq!(collection.by_id.len(), 1);
        assert_eq!(collection.get("c1").unwrap().name, "Replacement");
    }

    #[test]
    fn music_items_by_id_get_returns_item_or_none() {
        let collection = MusicItemsById::from(vec![make_compiler("c1", "Alpha")]);
        assert_eq!(collection.get("c1").unwrap().name, "Alpha");
        assert!(collection.get("missing").is_none());
    }

    #[test]
    fn sorted_by_name_normalised_sorts_both_directions() {
        let collection = MusicItemsById::from(vec![
            make_compiler("c1", "Zebra"),
            make_compiler("c2", "Alpha"),
            make_compiler("c3", "Miles"),
        ]);

        let ascending = collection.sorted_by_name_normalised(1);
        let ascending_ids: Vec<&str> = ascending.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ascending_ids, ["c2", "c3", "c1"]);

        // Direction 0 counts as ascending.
        let zero = collection.sorted_by_name_normalised(0);
        let zero_ids: Vec<&str> = zero.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(zero_ids, ["c2", "c3", "c1"]);

        let descending = collection.sorted_by_name_normalised(-1);
        let descending_ids: Vec<&str> = descending.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(descending_ids, ["c1", "c3", "c2"]);
    }

    #[test]
    fn partial_eq_compares_by_id_only() {
        let a = make_compiler("same-id", "Name A");
        let b = make_compiler("same-id", "Name B");
        let c = make_compiler("other-id", "Name A");
        assert_eq!(
            a, b,
            "items with equal ids are equal even when other fields differ"
        );
        assert_ne!(
            a, c,
            "items with different ids are not equal even when names match"
        );
    }

    #[test]
    fn track_serialises_id_as_underscore_id_and_omits_none_options() {
        let mut track = make_track("t1", "Song One");
        track.external_service_associations = Some(vec![ExternalServiceAssociation::Spotify {
            id: "spotify-track-1".to_string(),
            image_urls: None,
        }]);

        let json = serde_json::to_value(&track).unwrap();

        assert_eq!(json["_id"], "t1");
        assert!(
            json.get("id").is_none(),
            "id must serialise under the \"_id\" key only"
        );

        // None options are omitted entirely.
        for absent_key in [
            "disambiguation",
            "notes",
            "data_maybe_missing",
            "potential_duplicate",
            "needs_review",
            "album_id",
        ] {
            assert!(
                json.get(absent_key).is_none(),
                "expected {absent_key} to be omitted"
            );
        }

        // Non-optional and Some fields are present with their values.
        assert_eq!(json["name"], "Song One");
        assert_eq!(json["name_normalised"], "song one");
        assert_eq!(json["duration"], 215.5);
        assert_eq!(json["artist_ids"], serde_json::json!(["artist1"]));
        assert_eq!(
            json["external_service_associations"][0]["Spotify"]["id"],
            "spotify-track-1"
        );
    }

    #[test]
    fn track_round_trips_through_json() {
        let mut track = make_track("t1", "Song One");
        track.disambiguation = Some("the 1998 one".to_string());
        track.needs_review = Some(true);
        track.album_id = Some("album1".to_string());
        track.search_n_grams = vec!["so".to_string(), "son".to_string()];

        let json = serde_json::to_string(&track).unwrap();
        let back: Track = serde_json::from_str(&json).unwrap();

        // PartialEq only compares ids, so compare the interesting fields explicitly.
        assert_eq!(back.id, "t1");
        assert_eq!(back.name, "Song One");
        assert_eq!(back.name_normalised, "song one");
        assert_eq!(back.name_normalised_strong, "song one");
        assert_eq!(back.disambiguation.as_deref(), Some("the 1998 one"));
        assert_eq!(back.needs_review, Some(true));
        assert_eq!(back.album_id.as_deref(), Some("album1"));
        assert_eq!(back.duration, Some(215.5));
        assert_eq!(back.artist_ids, ["artist1"]);
        assert_eq!(back.search_terms, ["song one"]);
        assert_eq!(back.search_n_grams, ["so", "son"]);
    }

    #[test]
    fn track_missing_default_fields_deserialise_to_defaults() {
        // No artist_ids, album_id or duration in the document.
        let json = serde_json::json!({
            "_id": "t1",
            "name": "Song",
            "name_normalised": "song",
            "name_normalised_strong": "song",
            "search_terms": ["song"],
            "search_double_metaphone_codes": [],
            "search_n_grams": [],
        });
        let track: Track = serde_json::from_value(json).unwrap();
        assert_eq!(track.artist_ids, Vec::<String>::new());
        assert_eq!(track.album_id, None);
        assert_eq!(track.duration, None);
    }

    #[test]
    fn track_missing_required_field_fails_to_deserialise() {
        // search_terms has no default and is required.
        let json = serde_json::json!({
            "_id": "t1",
            "name": "Song",
            "name_normalised": "song",
            "name_normalised_strong": "song",
            "search_double_metaphone_codes": [],
            "search_n_grams": [],
        });
        let err = serde_json::from_value::<Track>(json).unwrap_err();
        assert!(
            err.to_string().contains("search_terms"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn playlist_serialises_id_as_underscore_id_and_omits_none_options() {
        let playlist = PlayList {
            id: "p1".to_string(),
            name: "Mix".to_string(),
            name_normalised: "mix".to_string(),
            name_normalised_strong: "mix".to_string(),
            disambiguation: None,
            notes: None,
            data_maybe_missing: None,
            potential_duplicate: None,
            needs_review: None,
            external_service_associations: None,
            search_terms: vec!["mix".to_string()],
            search_double_metaphone_codes: vec![],
            search_n_grams: vec![],
            compiler_ids: vec!["comp1".to_string()],
            track_ids: vec!["t1".to_string(), "t2".to_string()],
            duration: 431.0,
            user_id: "u1".to_string(),
            group_id: None,
            tag_ids: None,
            number: None,
            date: None,
        };

        let json = serde_json::to_value(&playlist).unwrap();
        assert_eq!(json["_id"], "p1");
        assert!(json.get("id").is_none());
        for absent_key in ["group_id", "tag_ids", "number", "date"] {
            assert!(
                json.get(absent_key).is_none(),
                "expected {absent_key} to be omitted"
            );
        }
        assert_eq!(json["user_id"], "u1");
        assert_eq!(json["duration"], 431.0);
        assert_eq!(json["track_ids"], serde_json::json!(["t1", "t2"]));
        assert_eq!(json["compiler_ids"], serde_json::json!(["comp1"]));
    }

    #[test]
    fn playlist_missing_default_fields_deserialise_to_defaults() {
        // No compiler_ids, track_ids or duration in the document.
        let json = serde_json::json!({
            "_id": "p1",
            "name": "Mix",
            "name_normalised": "mix",
            "name_normalised_strong": "mix",
            "search_terms": ["mix"],
            "search_double_metaphone_codes": [],
            "search_n_grams": [],
            "user_id": "u1",
        });
        let playlist: PlayList = serde_json::from_value(json).unwrap();
        assert_eq!(playlist.compiler_ids, Vec::<String>::new());
        assert_eq!(playlist.track_ids, Vec::<String>::new());
        assert_eq!(playlist.duration, 0.0);
        assert_eq!(playlist.user_id, "u1");
        assert_eq!(playlist.date, None);
    }

    #[test]
    fn playlist_missing_required_user_id_fails_to_deserialise() {
        let json = serde_json::json!({
            "_id": "p1",
            "name": "Mix",
            "name_normalised": "mix",
            "name_normalised_strong": "mix",
            "search_terms": ["mix"],
            "search_double_metaphone_codes": [],
            "search_n_grams": [],
        });
        let err = serde_json::from_value::<PlayList>(json).unwrap_err();
        assert!(
            err.to_string().contains("user_id"),
            "unexpected error: {err}"
        );
    }
}

#[cfg(feature = "server")]
pub mod server {
    use crate::database::ServerError;
    use mongodb::bson;

    pub async fn create_indexes<T>() -> Result<(), ServerError>
    where
        T: crate::models::MusicItem + Send + Sync + Unpin + for<'de> serde::Deserialize<'de>,
    {
        let database = crate::get_database().await?;
        let collection: mongodb::Collection<T> = database.collection(T::collection_name());
        let index_model = mongodb::IndexModel::builder()
            .keys(bson::doc! {"name_normalised": 1})
            .build();
        collection.create_index(index_model).await?;

        Ok(())
    }

    pub async fn load_music_items<T>(filter: bson::Document) -> Result<Vec<T>, ServerError>
    where
        T: crate::models::MusicItem + Send + Sync + Unpin + for<'de> serde::Deserialize<'de>,
    {
        use futures::stream::TryStreamExt;
        let database = crate::get_database().await?;
        let collection: mongodb::Collection<T> = database.collection(T::collection_name());
        let cursor = collection.find(filter).await?;
        let items = cursor.try_collect().await?;
        Ok(items)
    }
}
