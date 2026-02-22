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
