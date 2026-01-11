use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::music_data::album::Album;
use crate::music_data::track::Track;
#[cfg(feature = "server")]
use mongodb::bson;

crate::define_music_item_struct_with_common_fields!(Artist, "Artist", {
    // No additional fields
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistWithAssociatedData {
    pub tracks_by_id: HashMap<String, Track>,
    pub artists_by_id: HashMap<String, Artist>,
    pub albums_by_id: HashMap<String, Album>,
}

#[get("/api/artists/{artist_id}")]
pub async fn load_artist_with_associated_data(
    artist_id: String,
) -> Result<ArtistWithAssociatedData, ServerFnError> {
    // Load all tracks by this artist
    let tracks = crate::music_data::server::load_items::<Track>(bson::doc! {
        "artistIds": artist_id.clone()
    })
    .await?;

    let tracks_by_id: HashMap<String, Track> = tracks
        .into_iter()
        .map(|track| (track.id.clone(), track))
        .collect();

    if tracks_by_id.is_empty() {
        return Err(ServerFnError::new(format!(
            "Artist not found: {:?}",
            artist_id
        )));
    }

    // Load all artists for these tracks
    let artist_ids: Vec<String> = tracks_by_id
        .values()
        .flat_map(|track| track.artist_ids.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let artists_by_id = if !artist_ids.is_empty() {
        crate::music_data::server::load_items::<Artist>(bson::doc! {
            "_id": {"$in": artist_ids}
        })
        .await?
        .into_iter()
        .map(|artist| (artist.id.clone(), artist))
        .collect()
    } else {
        HashMap::new()
    };

    // Load all albums for these tracks
    let album_ids: Vec<String> = tracks_by_id
        .values()
        .filter_map(|track| track.album_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let albums_by_id = if !album_ids.is_empty() {
        crate::music_data::server::load_items::<Album>(bson::doc! {
            "_id": {"$in": album_ids}
        })
        .await?
        .into_iter()
        .map(|album| (album.id.clone(), album))
        .collect()
    } else {
        HashMap::new()
    };

    Ok(ArtistWithAssociatedData {
        tracks_by_id,
        artists_by_id,
        albums_by_id,
    })
}
