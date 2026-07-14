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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MusicItemsById;

    fn make_playlist(id: &str, date: Option<f64>) -> PlayList {
        PlayList {
            id: id.to_string(),
            name: format!("Playlist {id}"),
            name_normalised: format!("playlist {id}"),
            name_normalised_strong: format!("playlist {id}"),
            disambiguation: None,
            notes: None,
            data_maybe_missing: None,
            potential_duplicate: None,
            needs_review: None,
            external_service_associations: None,
            search_terms: vec![format!("playlist {id}")],
            search_double_metaphone_codes: vec![],
            search_n_grams: vec![],
            compiler_ids: vec![],
            track_ids: vec![],
            duration: 0.0,
            user_id: "u1".to_string(),
            group_id: None,
            tag_ids: None,
            number: None,
            date,
        }
    }

    fn ids(playlists: &[PlayList]) -> Vec<&str> {
        playlists.iter().map(|p| p.id.as_str()).collect()
    }

    #[test]
    fn vec_sorted_by_date_ascending() {
        let playlists = vec![
            make_playlist("p1", Some(2_000.0)),
            make_playlist("p2", Some(500.0)),
            make_playlist("p3", Some(1_000.0)),
        ];
        assert_eq!(ids(&playlists.sorted_by_date(1)), ["p2", "p3", "p1"]);
        // Direction 0 counts as ascending.
        assert_eq!(ids(&playlists.sorted_by_date(0)), ["p2", "p3", "p1"]);
        // The original vec is not reordered.
        assert_eq!(ids(&playlists), ["p1", "p2", "p3"]);
    }

    #[test]
    fn vec_sorted_by_date_descending() {
        let playlists = vec![
            make_playlist("p1", Some(2_000.0)),
            make_playlist("p2", Some(500.0)),
            make_playlist("p3", Some(1_000.0)),
        ];
        assert_eq!(ids(&playlists.sorted_by_date(-1)), ["p1", "p3", "p2"]);
    }

    #[test]
    fn sorted_by_date_treats_none_dates_as_zero() {
        // None sorts as 0.0: after a negative timestamp, before a positive one.
        let playlists = vec![
            make_playlist("positive", Some(100.0)),
            make_playlist("undated", None),
            make_playlist("negative", Some(-100.0)),
        ];
        assert_eq!(
            ids(&playlists.sorted_by_date(1)),
            ["negative", "undated", "positive"]
        );
        assert_eq!(
            ids(&playlists.sorted_by_date(-1)),
            ["positive", "undated", "negative"]
        );
    }

    #[test]
    fn music_items_by_id_sorted_by_date_both_directions() {
        let collection = MusicItemsById::from(vec![
            make_playlist("p1", Some(3_000.0)),
            make_playlist("p2", None),
            make_playlist("p3", Some(1_500.0)),
        ]);
        assert_eq!(ids(&collection.sorted_by_date(1)), ["p2", "p3", "p1"]);
        assert_eq!(ids(&collection.sorted_by_date(-1)), ["p1", "p3", "p2"]);
    }
}
