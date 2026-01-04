// #[cfg(feature = "ssr")]
// use leptos::logging::log;
// use leptos::prelude::*;
use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(Album, "Album", {
    #[serde(default)]
    artist_ids: Vec<String>,
});

// #[server]
// pub async fn load_albums(album_ids: Vec<String>) -> Result<Vec<Album>, crate::AppError> {
//     let result = crate::music_data::load_items_by_ids::<Album>("Album", album_ids.clone()).await;

//     if let Err(ref e) = result {
//         log!("Error loading albums for album IDs {:?}: {:?}", album_ids, e);
//     }
//     if let Ok(ref albums) = result {
//         log!("Loaded {} albums for album IDs {:?}", albums.len(), album_ids);
//     }
//     result.map_err(|e| crate::AppError::from(e))
// }
