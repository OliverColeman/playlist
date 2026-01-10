use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::music_data::album::Album;
use crate::music_data::artist::Artist;
#[cfg(feature = "server")]
use mongodb::bson;

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

/// A linked track document is used to group different versions of the same track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedTrack {
    #[serde(rename = "_id")]
    pub id: String,
    pub track_name_normalised_strong: String,
    pub track_ids: Vec<String>,
    pub artist_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackWithAssociatedData {
    pub linked_tracks_by_id: HashMap<String, Track>,
    pub artists_by_id: HashMap<String, Artist>,
    pub albums_by_id: HashMap<String, Album>,
}

#[get("/api/tracks/{track_id}")]
pub async fn load_track_with_associated_data(
    track_id: String,
) -> Result<TrackWithAssociatedData, ServerFnError> {
    // Load the "linked tracks" document to get the different versions of the same track, if any
    let linked_tracks_doc: std::result::Result<Option<LinkedTrack>, ServerFnError> = {
        use crate::server::ServerError;
        let database = crate::server::get_database().await?;
        let collection: mongodb::Collection<LinkedTrack> = database.collection("LinkedTrack");
        let index_model = mongodb::IndexModel::builder()
            .keys(bson::doc! {"track_ids": 1})
            .build();
        collection
            .create_index(index_model)
            .await
            .map_err(|e| ServerError::DatabaseError(e))?;

        let linked_track = collection
            .find_one(bson::doc! {"trackIds": &track_id})
            .await
            .map_err(|e| ServerError::DatabaseError(e))?;

        Ok(linked_track)
    };

    // log the linked tracks document for debugging
    tracing::info!("Linked tracks doc: {:?}", linked_tracks_doc);

    // If there are linked tracks, load them all, otherwise just load the given track
    let track_ids = match linked_tracks_doc? {
        Some(doc) => doc.track_ids,
        None => vec![track_id],
    };

    let linked_tracks_by_id = crate::music_data::server::load_items::<Track>(bson::doc! {
        "_id": {"$in": track_ids.clone()}
    })
    .await?
    .into_iter()
    .map(|track| (track.id.clone(), track))
    .collect::<HashMap<String, Track>>();

    // Load all artists and albums for all tracks
    let mut artist_ids = vec![];
    let mut album_ids = vec![];
    for track in linked_tracks_by_id.values() {
        for artist_id in &track.artist_ids {
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
    let artists_by_id = crate::music_data::server::load_items::<Artist>(bson::doc! {
        "_id": {"$in": artist_ids.clone()}
    })
    .await?
    .into_iter()
    .map(|artist| (artist.id.clone(), artist))
    .collect::<HashMap<String, Artist>>();

    let albums_by_id = crate::music_data::server::load_items::<Album>(bson::doc! {
        "_id": {"$in": album_ids.clone()}
    })
    .await?
    .into_iter()
    .map(|album| (album.id.clone(), album))
    .collect::<HashMap<String, Album>>();

    Ok(TrackWithAssociatedData {
        linked_tracks_by_id,
        artists_by_id,
        albums_by_id,
    })
}
