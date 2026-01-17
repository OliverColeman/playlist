use serde::{Deserialize, Serialize};

pub mod album;
pub mod artist;
pub mod compiler;
pub mod playlist;
pub mod track;
pub const JD_GROUP_ID: &str = "zmWKoBuAoSLCWDvzn";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large: Option<String>,
}

/// Trait for all music items with common fields
pub trait MusicItemBase {
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
    fn image_urls(&self) -> Option<&ImageUrls>;
    fn spotify_id(&self) -> Option<&str>;
    fn mb_id(&self) -> Option<&str>;
    fn soundex(&self) -> &[String];
    fn double_metaphone(&self) -> Option<&[String]>;
}

pub trait MusicItem: MusicItemBase + Clone {}

#[macro_export]
macro_rules! define_music_item_struct_with_common_fields {
    ($name:ident, $collection_name:expr, { $($(#[$attr:meta])* $field_name:ident : $field_type:ty),* $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct $name {
            #[serde(rename = "_id")]
            pub id: String,

            pub name: String,

            /// Normalized version of the name (from normaliseString)
            pub name_normalised: String,

            /// Used for LinkedT matching (from normaliseStringStrong)
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
            pub image_urls: Option<crate::models::ImageUrls>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub spotify_id: Option<String>,

            /// MusicBrainz ID
            #[serde(skip_serializing_if = "Option::is_none")]
            pub mb_id: Option<String>,

            #[serde(default)]
            pub soundex: Vec<String>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub double_metaphone: Option<Vec<String>>,

            $($(#[$attr])* pub $field_name: $field_type,)*
        }

        impl crate::models::MusicItemBase for $name {
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
            fn image_urls(&self) -> Option<&crate::models::ImageUrls> { self.image_urls.as_ref() }
            fn spotify_id(&self) -> Option<&str> { self.spotify_id.as_deref() }
            fn mb_id(&self) -> Option<&str> { self.mb_id.as_deref() }
            fn soundex(&self) -> &[String] { &self.soundex }
            fn double_metaphone(&self) -> Option<&[String]> { self.double_metaphone.as_deref() }
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

    pub async fn load_items<T>(filter: bson::Document) -> Result<Vec<T>, ServerError>
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
