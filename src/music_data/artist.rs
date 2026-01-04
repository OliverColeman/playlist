// #[cfg(feature = "ssr")]
// use leptos::logging::log;
// use leptos::prelude::*;
use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(Artist, "Artist", {
    // No additional fields
});

// #[server]
// pub async fn load_artists(artist_ids: Vec<String>) -> Result<Vec<Artist>, crate::AppError> {
//     let result = crate::music_data::load_items_by_ids::<Artist>("Artist", artist_ids.clone()).await;

//     if let Err(ref e) = result {
//         log!("Error loading artists for artist IDs {:?}: {:?}", artist_ids, e);
//     }
//     if let Ok(ref artists) = result {
//         log!("Loaded {} artists for artist IDs {:?}", artists.len(), artist_ids);
//     }
//     result.map_err(|e| crate::AppError::from(e))
// }
