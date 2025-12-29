#[cfg(feature = "ssr")]
use crate::music_data::JD_GROUP_ID;
#[cfg(feature = "ssr")]
use leptos::logging::log;
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use mongodb::bson;
use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(PlayList, {
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

    #[serde(skip_serializing_if = "Option::is_none")]
    spotify_user_id: Option<String>,
});

impl PartialEq for PlayList {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[server]
pub async fn load_playlists() -> Result<Vec<PlayList>, crate::AppError> {
    let result: Result<Vec<PlayList>, crate::ssr::ServerError> = async {
        use futures::stream::TryStreamExt;
        let database = crate::ssr::get_database().await?;
        let collection: mongodb::Collection<PlayList> = database.collection("PlayList");
        let cursor = collection.find(bson::doc! {"groupId": JD_GROUP_ID}).await?;
        let playlists = cursor.try_collect().await?;
        Ok(playlists)
    }
    .await;
    // Log error if any
    if let Err(ref e) = result {
        log!("Error loading playlists: {:?}", e);
    }
    // Log length of playlists loaded
    if let Ok(ref playlists) = result {
        log!("Loaded {} playlists", playlists.len());
    }
    result.map_err(|e| crate::AppError::from(e))
}
