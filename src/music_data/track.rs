#[cfg(feature = "ssr")]
use leptos::logging::log;
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use mongodb::bson;
use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(Track, {
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

impl PartialEq for Track {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[server]
pub async fn load_playlist_tracks(track_ids: Vec<String>) -> Result<Vec<Track>, crate::AppError> {
    let result: Result<Vec<Track>, crate::ssr::ServerError> = async {
        use futures::stream::TryStreamExt;
        let database = crate::ssr::get_database().await?;
        let track_collection: mongodb::Collection<Track> = database.collection("Track");
        let cursor = track_collection
            .find(bson::doc! {"_id": {"$in": track_ids.clone()}})
            .await?;
        let tracks = cursor.try_collect().await?;
        Ok(tracks)
    }
    .await;
    if let Err(ref e) = result {
        log!("Error loading tracks for track IDs {:?}: {:?}", track_ids, e);
    }
    if let Ok(ref tracks) = result {
        log!("Loaded {} tracks for track IDs {:?}", tracks.len(), track_ids);
    }
    result.map_err(|e| crate::AppError::from(e))
}
