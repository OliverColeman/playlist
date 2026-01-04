// #[cfg(feature = "ssr")]
// use leptos::logging::log;
// use leptos::prelude::*;
use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(Track, "Track", {
    #[serde(default)]
    artist_ids: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    album_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<f64>,

    #[serde(default)]
    appears_in_play_lists: Vec<String>,

    #[serde(default)]
    appears_in_play_list_groups: Vec<String>,
});

// #[server]
// pub async fn load_tracks(track_ids: Vec<String>) -> Result<Vec<Track>, crate::AppError> {
//     let result = crate::music_data::load_items_by_ids::<Track>("Track", track_ids.clone()).await;

//     if let Err(ref e) = result {
//         log!("Error loading tracks for track IDs {:?}: {:?}", track_ids, e);
//     }
//     if let Ok(ref tracks) = result {
//         log!("Loaded {} tracks for track IDs {:?}", tracks.len(), track_ids);
//     }
//     result.map_err(|e| crate::AppError::from(e))
// }
