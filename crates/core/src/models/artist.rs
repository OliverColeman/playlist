use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::models::album::Album;
use crate::models::track::Track;

crate::define_music_item_struct_with_common_fields!(Artist, "artist", {
    #[serde(skip_serializing_if = "Option::is_none")]
    alt_names: Option<Vec<String>>,
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistWithAssociatedData {
    pub tracks_by_id: HashMap<String, Track>,
    pub linked_tracks: Vec<HashSet<String>>,
    pub artists_by_id: HashMap<String, Artist>,
    pub albums_by_id: HashMap<String, Album>,
}
