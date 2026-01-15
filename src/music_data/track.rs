use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::music_data::album::Album;
use crate::music_data::artist::Artist;
use crate::music_data::playlist::PlayList;
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

#[cfg(feature = "server")]
pub async fn load_linked_tracks(query: bson::Document) -> Result<Vec<LinkedTrack>, ServerFnError> {
    use crate::server::ServerError;
    use futures::stream::TryStreamExt;

    let database = crate::server::get_database().await?;
    let collection: mongodb::Collection<LinkedTrack> = database.collection("LinkedTrack");

    // Create index for track_ids queries (won't hurt if it already exists)
    let index_model = mongodb::IndexModel::builder()
        .keys(bson::doc! {"track_ids": 1})
        .build();
    collection
        .create_index(index_model)
        .await
        .map_err(|e| ServerError::DatabaseError(e))?;

    let cursor = collection
        .find(query)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let linked_tracks: Vec<LinkedTrack> = cursor
        .try_collect()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(linked_tracks)
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
    let linked_tracks_docs = load_linked_tracks(bson::doc! {"trackIds": &track_id}).await?;
    let linked_tracks_doc = linked_tracks_docs.into_iter().next();

    // If there are linked tracks, load them all, otherwise just load the given track
    let track_ids = match linked_tracks_doc {
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

    if linked_tracks_by_id.is_empty() {
        return Err(ServerFnError::new(format!(
            "Track not found: {:?}",
            track_ids
        )));
    }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopularTracksData {
    pub sorted_track_ids: Vec<String>,
    pub tracks_by_id: HashMap<String, Track>,
    pub linked_tracks: Vec<HashSet<String>>,
    pub artists_by_id: HashMap<String, Artist>,
    pub albums_by_id: HashMap<String, Album>,
}

#[get("/api/tracks/popular")]
pub async fn load_popular_tracks() -> Result<PopularTracksData, ServerFnError> {
    // Get all JD playlists
    let playlists = crate::music_data::server::load_items::<PlayList>(
        bson::doc! {"groupId": crate::music_data::JD_GROUP_ID},
    )
    .await?;

    // Load all LinkedTrack documents
    let linked_tracks = load_linked_tracks(bson::doc! {}).await?;
    // Create a map from every linked track id to all the track ids linked to it
    let mut linked_tracks_map: HashMap<String, &LinkedTrack> = HashMap::new();
    for linked_track in &linked_tracks {
        for track_id in &linked_track.track_ids {
            linked_tracks_map.insert(track_id.clone(), linked_track);
        }
    }

    let mut track_count_map: HashMap<String, usize> = HashMap::new();
    for playlist in playlists {
        for track_id in playlist.track_ids {
            *track_count_map.entry(track_id).or_insert(0) += 1;
        }
    }

    // Sort all tracks by popularity, ignoring linked versions for now
    let mut all_track_count_sorted: Vec<(String, usize)> =
        track_count_map.clone().into_iter().collect();
    all_track_count_sorted.sort_by(|a, b| a.1.cmp(&b.1));

    // Update track_count_map to add counts from linked tracks and remove the less popular versions
    for (track_id, count) in &all_track_count_sorted {
        if let Some(linked_tracks) = linked_tracks_map.get(track_id) {
            for linked_track_id in &linked_tracks.track_ids {
                if linked_track_id != track_id {
                    let linked_count = track_count_map.remove(linked_track_id).unwrap_or(0);
                    if linked_count > 0 {
                        *track_count_map.entry(track_id.clone()).or_insert(0) += linked_count;
                    }
                }
            }
        }
    }

    // Now get the top 100 tracks, including the counts from linked versions
    let mut track_count_sorted: Vec<(String, usize)> = track_count_map.into_iter().collect();
    track_count_sorted.sort_by(|a, b| b.1.cmp(&a.1));
    track_count_sorted.truncate(100);
    let sorted_track_ids = track_count_sorted
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<Vec<String>>();

    // Load the tracks
    let tracks_by_id: HashMap<String, Track> =
        crate::music_data::server::load_items::<Track>(bson::doc! {
            "_id": {"$in": sorted_track_ids.clone()}
        })
        .await?
        .into_iter()
        .map(|track| (track.id.clone(), track))
        .collect();

    // Load all artists and albums for these tracks
    let mut artist_ids: HashSet<String> = HashSet::new();
    let mut album_ids: HashSet<String> = HashSet::new();
    for track in tracks_by_id.values() {
        for artist_id in &track.artist_ids {
            artist_ids.insert(artist_id.clone());
        }
        if let Some(album_id) = &track.album_id {
            album_ids.insert(album_id.clone());
        }
    }

    let artists_by_id = if !artist_ids.is_empty() {
        crate::music_data::server::load_items::<Artist>(bson::doc! {
            "_id": {"$in": artist_ids.into_iter().collect::<Vec<String>>()}
        })
        .await?
        .into_iter()
        .map(|artist| (artist.id.clone(), artist))
        .collect()
    } else {
        HashMap::new()
    };

    let albums_by_id = if !album_ids.is_empty() {
        crate::music_data::server::load_items::<Album>(bson::doc! {
            "_id": {"$in": album_ids.into_iter().collect::<Vec<String>>()}
        })
        .await?
        .into_iter()
        .map(|album| (album.id.clone(), album))
        .collect()
    } else {
        HashMap::new()
    };

    let linked_tracks = sorted_track_ids
        .iter()
        .map(|track_id| {
            if let Some(linked_track) = linked_tracks_map.get(track_id) {
                linked_track
                    .track_ids
                    .iter()
                    .cloned()
                    .collect::<HashSet<String>>()
            } else {
                let mut hs = HashSet::new();
                hs.insert(track_id.clone());
                hs
            }
        })
        .collect::<Vec<HashSet<String>>>();

    tracing::info!("linked_tracks: {:?}", linked_tracks);

    Ok(PopularTracksData {
        sorted_track_ids,
        tracks_by_id,
        linked_tracks,
        artists_by_id,
        albums_by_id,
    })
}
