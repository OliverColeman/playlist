use dioxus::prelude::*;
use std::collections::HashMap;

use crate::music_data::artist::Artist;
use crate::music_data::track::Track;
use crate::music_data::{self, album::Album};
#[cfg(feature = "server")]
use mongodb::bson;
use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(PlayList, "PlayList", {
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

pub trait PlaylistCollection {
    fn sorted_by_date(&self, direction: i8) -> Vec<PlayList>;
}

impl PlaylistCollection for music_data::MusicItemsById<PlayList> {
    fn sorted_by_date(&self, direction: i8) -> Vec<PlayList> {
        let mut items: Vec<PlayList> = self.by_id.values().cloned().collect();
        items.sort_by(|a, b| {
            let a_date = a.date.unwrap_or(0.0);
            let b_date = b.date.unwrap_or(0.0);
            if direction >= 0 {
                a_date
                    .partial_cmp(&b_date)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                b_date
                    .partial_cmp(&a_date)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });
        items
    }
}

#[get("/api/playlists")]
pub async fn load_playlists() -> Result<Vec<PlayList>, crate::ServerFnError> {
    let result = crate::music_data::server::load_items::<PlayList>(
        bson::doc! {"groupId": crate::music_data::JD_GROUP_ID},
    )
    .await;

    if let Err(ref e) = result {
        tracing::info!("Error loading playlists: {:?}", e);
    }
    if let Ok(ref playlists) = result {
        tracing::info!("Loaded {} playlists", playlists.len());
    }

    // simulate a longer load time for demonstration purposes
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    Ok(result?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayListWithAssociatedData {
    pub playlist: PlayList,
    pub tracks_by_id: HashMap<String, Track>,
    pub artists_by_id: HashMap<String, Artist>,
    pub albums_by_id: HashMap<String, Album>,
}

#[get("/api/playlists/{playlist_id}")]
pub async fn load_playlist_with_associated_data(
    playlist_id: String,
) -> Result<PlayListWithAssociatedData, ServerFnError> {
    let playlist = crate::music_data::server::load_items::<PlayList>(
        bson::doc! { "_id": playlist_id.clone() },
    )
    .await?
    .into_iter()
    .next()
    .ok_or_else(|| ServerFnError::new(format!("Playlist {} not found", playlist_id)))?;

    let track_ids = playlist.track_ids.clone();
    let tracks = crate::music_data::server::load_items::<Track>(
        bson::doc! {"_id": {"$in": track_ids.clone()}},
    )
    .await?;
    let tracks_by_id: HashMap<String, Track> =
        tracks.into_iter().map(|t| (t.id.clone(), t)).collect();
    let mut artist_ids: Vec<String> = Vec::new();
    let mut album_ids: Vec<String> = Vec::new();
    for track_id in track_ids.iter() {
        if let Some(track) = tracks_by_id.get(track_id) {
            for artist_id in track.artist_ids.iter() {
                if !artist_ids.contains(artist_id) {
                    artist_ids.push(artist_id.clone());
                }
            }
            if let Some(album_id) = &track.album_id {
                if !album_ids.contains(album_id) {
                    album_ids.push(album_id.clone());
                }
            }
        }
    }
    let artists = crate::music_data::server::load_items::<Artist>(
        bson::doc! {"_id": {"$in": artist_ids.clone()}},
    )
    .await?;
    let artists_by_id: HashMap<String, Artist> =
        artists.into_iter().map(|a| (a.id.clone(), a)).collect();
    let albums = crate::music_data::server::load_items::<Album>(
        bson::doc! {"_id": {"$in": album_ids.clone()}},
    )
    .await?;
    let albums_by_id: HashMap<String, Album> =
        albums.into_iter().map(|a| (a.id.clone(), a)).collect();
    Ok(PlayListWithAssociatedData {
        playlist,
        tracks_by_id,
        artists_by_id,
        albums_by_id,
    })
}
