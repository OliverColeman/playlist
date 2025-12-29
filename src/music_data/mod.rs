use serde::{Deserialize, Serialize};

pub mod compiler;
pub mod playlist;
pub mod track;

pub const JD_GROUP_ID: &str = "zmWKoBuAoSLCWDvzn";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageUrls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large: Option<String>,
}

/// Trait for all music items with common fields
pub trait MusicItem {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn name_normalised(&self) -> &str;
    fn name_normalised_strong(&self) -> Option<&str>;
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

#[macro_export]
macro_rules! define_music_item_struct_with_common_fields {
    ($name:ident, { $($(#[$attr:meta])* $field_name:ident : $field_type:ty),* $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            #[serde(rename = "_id")]
            pub id: String,

            pub name: String,

            /// Normalized version of the name (from normaliseString)
            pub name_normalised: String,

            /// Used for LinkedTrack matching (from normaliseStringStrong)
            #[serde(skip_serializing_if = "Option::is_none")]
            pub name_normalised_strong: Option<String>,

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
            pub image_urls: Option<crate::music_data::ImageUrls>,

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

        impl crate::music_data::MusicItem for $name {
            fn id(&self) -> &str { &self.id }
            fn name(&self) -> &str { &self.name }
            fn name_normalised(&self) -> &str { &self.name_normalised }
            fn name_normalised_strong(&self) -> Option<&str> { self.name_normalised_strong.as_deref() }
            fn disambiguation(&self) -> Option<&str> { self.disambiguation.as_deref() }
            fn notes(&self) -> Option<&str> { self.notes.as_deref() }
            fn data_maybe_missing(&self) -> Option<&[String]> { self.data_maybe_missing.as_deref() }
            fn potential_duplicate(&self) -> Option<bool> { self.potential_duplicate }
            fn needs_review(&self) -> Option<bool> { self.needs_review }
            fn image_urls(&self) -> Option<&crate::music_data::ImageUrls> { self.image_urls.as_ref() }
            fn spotify_id(&self) -> Option<&str> { self.spotify_id.as_deref() }
            fn mb_id(&self) -> Option<&str> { self.mb_id.as_deref() }
            fn soundex(&self) -> &[String] { &self.soundex }
            fn double_metaphone(&self) -> Option<&[String]> { self.double_metaphone.as_deref() }
        }
    };
}
