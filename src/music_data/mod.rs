use serde::{Deserialize, Serialize};

pub mod playlist;

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
    };
}
