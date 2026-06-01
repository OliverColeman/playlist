use std::collections::{HashMap, HashSet};

use crate::models::album::Album;
use crate::models::artist::Artist;
use crate::models::track::Track;
use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(
    PlayList, crate::models::ItemType::Playlist, "playlist",
    {
        #[serde(default)]
        compiler_ids: Vec<String>,

        #[serde(default)]
        track_ids: Vec<String>,

        #[serde(default)]
        duration: f64,

        user_id: String,

        #[serde(skip_serializing_if = "Option::is_none")]
        group_id: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        tag_ids: Option<Vec<String>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        number: Option<u64>,

        /// Unix timestamp
        #[serde(skip_serializing_if = "Option::is_none")]
        date: Option<f64>,
    }
);

pub trait PlaylistCollection {
    fn sorted_by_date(&self, direction: i8) -> Vec<PlayList>;
}

impl PlaylistCollection for Vec<PlayList> {
    fn sorted_by_date(&self, direction: i8) -> Vec<PlayList> {
        let mut items: Vec<PlayList> = self.clone();
        items.sort_by(|a, b| {
            let a_date = a.date.unwrap_or(0.0);
            let b_date = b.date.unwrap_or(0.0);
            if direction >= 0 {
                a_date.partial_cmp(&b_date).unwrap()
            } else {
                b_date.partial_cmp(&a_date).unwrap()
            }
        });
        items
    }
}

impl PlaylistCollection for crate::models::MusicItemsById<PlayList> {
    fn sorted_by_date(&self, direction: i8) -> Vec<PlayList> {
        self.by_id
            .values()
            .cloned()
            .collect::<Vec<PlayList>>()
            .sorted_by_date(direction)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayListWithAssociatedData {
    pub playlist: PlayList,
    pub tracks_by_id: HashMap<String, Track>,
    pub linked_tracks: Vec<HashSet<String>>,
    pub artists_by_id: HashMap<String, Artist>,
    pub albums_by_id: HashMap<String, Album>,
}
