use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

// Helper function to convert core errors to ServerFnError (server only)
#[cfg(feature = "server")]
fn to_server_error(err: playlist_core::database::ServerError) -> ServerFnError {
    ServerFnError::new(format!("{:?}", err))
}

// Re-export for convenience
pub use playlist_core::models::{
    album::Album,
    artist::{Artist, ArtistWithAssociatedData},
    compiler::Compiler,
    playlist::{PlayList, PlayListWithAssociatedData},
    track::{LinkedTrack, PopularTracksData, Track, TrackWithAssociatedData},
};

/// Load all compilers
#[get("/api/compilers")]
pub async fn load_compilers() -> Result<Vec<Compiler>, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_items;

    let result = load_items::<Compiler>(bson::doc! {})
        .await
        .map_err(to_server_error)?;
    Ok(result)
}

/// Load all playlists
#[get("/api/playlists")]
pub async fn load_playlists() -> Result<Vec<PlayList>, ServerFnError> {
    tracing::info!("Loading all playlists");
    use mongodb::bson;
    use playlist_core::models::server::load_items;

    let playlists = load_items::<PlayList>(bson::doc! {})
        .await
        .map_err(to_server_error)?;
    tracing::info!("Loaded {} playlists", playlists.len());
    Ok(playlists)
}

/// Load a specific playlist with associated data
#[get("/api/playlists/{playlist_id}")]
pub async fn load_playlist_with_associated_data(
    playlist_id: String,
) -> Result<PlayListWithAssociatedData, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_items;
    use playlist_core::models::track::load_linked_tracks;

    // Load the playlist
    let playlists = load_items::<PlayList>(bson::doc! {"_id": &playlist_id})
        .await
        .map_err(to_server_error)?;
    let playlist = playlists
        .into_iter()
        .next()
        .ok_or_else(|| ServerFnError::new(format!("Playlist not found: {}", playlist_id)))?;

    // Load the tracks
    let tracks_by_id = if !playlist.track_ids.is_empty() {
        load_items::<Track>(bson::doc! {
            "_id": {"$in": &playlist.track_ids}
        })
        .await
        .map_err(to_server_error)?
        .into_iter()
        .map(|track| (track.id.clone(), track))
        .collect()
    } else {
        HashMap::new()
    };

    // Load linked tracks
    let linked_tracks: Vec<HashSet<String>> = load_linked_tracks(bson::doc! {})
        .await
        .map_err(to_server_error)?
        .into_iter()
        .map(|lt| lt.track_ids.into_iter().collect())
        .collect();

    // Load all artists and albums
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
        load_items::<Artist>(bson::doc! {
            "_id": {"$in": artist_ids.into_iter().collect::<Vec<String>>()}
        })
        .await
        .map_err(to_server_error)?
        .into_iter()
        .map(|artist| (artist.id.clone(), artist))
        .collect()
    } else {
        HashMap::new()
    };

    let albums_by_id = if !album_ids.is_empty() {
        load_items::<Album>(bson::doc! {
            "_id": {"$in": album_ids.into_iter().collect::<Vec<String>>()}
        })
        .await
        .map_err(to_server_error)?
        .into_iter()
        .map(|album| (album.id.clone(), album))
        .collect()
    } else {
        HashMap::new()
    };

    Ok(PlayListWithAssociatedData {
        playlist,
        tracks_by_id,
        linked_tracks,
        artists_by_id,
        albums_by_id,
    })
}

/// Load artist with associated data
#[get("/api/artists/{artist_id}")]
pub async fn load_artist_with_associated_data(
    artist_id: String,
) -> Result<ArtistWithAssociatedData, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_items;
    use playlist_core::models::track::load_linked_tracks;

    // Load all tracks by this artist
    let tracks = load_items::<Track>(bson::doc! {
        "artist_ids": artist_id.clone()
    })
    .await
    .map_err(to_server_error)?;

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

    let linked_tracks: Vec<HashSet<String>> =
        load_linked_tracks(bson::doc! {"artist_ids": artist_id.clone()})
            .await
            .map_err(to_server_error)?
            .into_iter()
            .map(|lt| lt.track_ids.into_iter().collect())
            .collect();

    // Load all artists for all tracks
    let mut artist_ids: Vec<String> = vec![];
    let mut album_ids: Vec<String> = vec![];
    for track in tracks_by_id.values() {
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

    let artists_by_id = load_items::<Artist>(bson::doc! {
        "_id": {"$in": artist_ids.clone()}
    })
    .await
    .map_err(to_server_error)?
    .into_iter()
    .map(|artist| (artist.id.clone(), artist))
    .collect::<HashMap<String, Artist>>();

    let albums_by_id = load_items::<Album>(bson::doc! {
        "_id": {"$in": album_ids.clone()}
    })
    .await
    .map_err(to_server_error)?
    .into_iter()
    .map(|album| (album.id.clone(), album))
    .collect::<HashMap<String, Album>>();

    Ok(ArtistWithAssociatedData {
        tracks_by_id,
        linked_tracks,
        artists_by_id,
        albums_by_id,
    })
}

/// Load track with associated data
#[get("/api/tracks/{track_id}")]
pub async fn load_track_with_associated_data(
    track_id: String,
) -> Result<TrackWithAssociatedData, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_items;
    use playlist_core::models::track::load_linked_tracks;

    // Load the "linked tracks" document to get the different versions of the same track, if any
    let linked_tracks_docs = load_linked_tracks(bson::doc! {"track_ids": &track_id})
        .await
        .map_err(to_server_error)?;
    let linked_tracks_doc = linked_tracks_docs.into_iter().next();

    // If there are linked tracks, load them all, otherwise just load the given track
    let track_ids = match linked_tracks_doc {
        Some(doc) => doc.track_ids,
        None => vec![track_id],
    };

    let linked_tracks_by_id = load_items::<Track>(bson::doc! {
        "_id": {"$in": track_ids.clone()}
    })
    .await
    .map_err(to_server_error)?
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
    let mut artist_ids: Vec<String> = vec![];
    let mut album_ids: Vec<String> = vec![];
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
    let artists_by_id = load_items::<Artist>(bson::doc! {
        "_id": {"$in": artist_ids.clone()}
    })
    .await
    .map_err(to_server_error)?
    .into_iter()
    .map(|artist| (artist.id.clone(), artist))
    .collect::<HashMap<String, Artist>>();

    let albums_by_id = load_items::<Album>(bson::doc! {
        "_id": {"$in": album_ids.clone()}
    })
    .await
    .map_err(to_server_error)?
    .into_iter()
    .map(|album| (album.id.clone(), album))
    .collect::<HashMap<String, Album>>();

    Ok(TrackWithAssociatedData {
        linked_tracks_by_id,
        artists_by_id,
        albums_by_id,
    })
}

/// Load popular tracks
#[get("/api/tracks/popular")]
pub async fn load_popular_tracks() -> Result<PopularTracksData, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_items;
    use playlist_core::models::track::load_linked_tracks;

    // Get all JD playlists
    let playlists =
        load_items::<PlayList>(bson::doc! {"group_id": playlist_core::models::JD_GROUP_ID})
            .await
            .map_err(to_server_error)?;

    // Load all LinkedTrack documents
    let linked_tracks = load_linked_tracks(bson::doc! {})
        .await
        .map_err(to_server_error)?;
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
    for (track_id, _count) in &all_track_count_sorted {
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
    let tracks_by_id: HashMap<String, Track> = load_items::<Track>(bson::doc! {
        "_id": {"$in": sorted_track_ids.clone()}
    })
    .await
    .map_err(to_server_error)?
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
        load_items::<Artist>(bson::doc! {
            "_id": {"$in": artist_ids.into_iter().collect::<Vec<String>>()}
        })
        .await
        .map_err(to_server_error)?
        .into_iter()
        .map(|artist| (artist.id.clone(), artist))
        .collect()
    } else {
        HashMap::new()
    };

    let albums_by_id = if !album_ids.is_empty() {
        load_items::<Album>(bson::doc! {
            "_id": {"$in": album_ids.into_iter().collect::<Vec<String>>()}
        })
        .await
        .map_err(to_server_error)?
        .into_iter()
        .map(|album| (album.id.clone(), album))
        .collect()
    } else {
        HashMap::new()
    };

    let linked_tracks = sorted_track_ids
        .iter()
        .map(|track_id| {
            linked_tracks_map
                .get(track_id)
                .map(|lt| lt.track_ids.iter().cloned().collect())
                .unwrap_or_else(|| HashSet::from([track_id.clone()]))
        })
        .collect();

    Ok(PopularTracksData {
        sorted_track_ids,
        tracks_by_id,
        linked_tracks,
        artists_by_id,
        albums_by_id,
    })
}
