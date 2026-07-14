use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// Helper function to convert core errors to ServerFnError (server only)
#[cfg(feature = "server")]
fn to_server_error(err: playlist_core::database::ServerError) -> ServerFnError {
    ServerFnError::new(format!("{:?}", err))
}

// Helper function to build a "not found" ServerFnError that surfaces as HTTP 404 (server only).
// The `code` field of `ServerFnError::ServerError` becomes the HTTP response status (see
// `impl IntoResponse for ServerFnError` in dioxus-fullstack-core).
#[cfg(feature = "server")]
fn not_found_error(message: String) -> ServerFnError {
    ServerFnError::ServerError {
        message,
        code: 404,
        details: None,
    }
}

// Re-export for convenience
pub use playlist_core::models::{
    album::Album,
    artist::{Artist, ArtistWithAssociatedData},
    compiler::Compiler,
    playlist::{PlayList, PlayListWithAssociatedData},
    track::{LinkedTrack, Track, TrackListWithAssociatedData, TrackWithAssociatedData},
};

/// Load all compilers
#[get("/api/compilers")]
pub async fn load_compilers() -> Result<Vec<Compiler>, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_music_items;

    let result = load_music_items::<Compiler>(bson::doc! {})
        .await
        .map_err(to_server_error)?;
    Ok(result)
}

/// Load all playlists
#[get("/api/playlists")]
pub async fn load_playlists() -> Result<Vec<PlayList>, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_music_items;

    let playlists = load_music_items::<PlayList>(bson::doc! {})
        .await
        .map_err(to_server_error)?;
    Ok(playlists)
}

/// Load a specific playlist with associated data
#[get("/api/playlists/{playlist_id}")]
pub async fn load_playlist_with_associated_data(
    playlist_id: String,
) -> Result<PlayListWithAssociatedData, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_music_items;
    use playlist_core::models::track::load_linked_tracks;

    // Load the playlist
    let playlists = load_music_items::<PlayList>(bson::doc! {"_id": &playlist_id})
        .await
        .map_err(to_server_error)?;
    let playlist = playlists
        .into_iter()
        .next()
        .ok_or_else(|| not_found_error(format!("Playlist not found: {}", playlist_id)))?;

    // Load the tracks
    let tracks_by_id = if !playlist.track_ids.is_empty() {
        load_music_items::<Track>(bson::doc! {
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
        load_music_items::<Artist>(bson::doc! {
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
        load_music_items::<Album>(bson::doc! {
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
    use playlist_core::models::server::load_music_items;
    use playlist_core::models::track::load_linked_tracks;

    // Load all tracks by this artist
    let tracks = load_music_items::<Track>(bson::doc! {
        "artist_ids": artist_id.clone()
    })
    .await
    .map_err(to_server_error)?;

    let tracks_by_id: HashMap<String, Track> = tracks
        .into_iter()
        .map(|track| (track.id.clone(), track))
        .collect();

    if tracks_by_id.is_empty() {
        return Err(not_found_error(format!(
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

    let artists_by_id = load_music_items::<Artist>(bson::doc! {
        "_id": {"$in": artist_ids.clone()}
    })
    .await
    .map_err(to_server_error)?
    .into_iter()
    .map(|artist| (artist.id.clone(), artist))
    .collect::<HashMap<String, Artist>>();

    let albums_by_id = load_music_items::<Album>(bson::doc! {
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
    use playlist_core::models::server::load_music_items;
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

    let linked_tracks_by_id = load_music_items::<Track>(bson::doc! {
        "_id": {"$in": track_ids.clone()}
    })
    .await
    .map_err(to_server_error)?
    .into_iter()
    .map(|track| (track.id.clone(), track))
    .collect::<HashMap<String, Track>>();

    if linked_tracks_by_id.is_empty() {
        return Err(not_found_error(format!("Track not found: {:?}", track_ids)));
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
    let artists_by_id = load_music_items::<Artist>(bson::doc! {
        "_id": {"$in": artist_ids.clone()}
    })
    .await
    .map_err(to_server_error)?
    .into_iter()
    .map(|artist| (artist.id.clone(), artist))
    .collect::<HashMap<String, Artist>>();

    let albums_by_id = load_music_items::<Album>(bson::doc! {
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
pub async fn load_popular_tracks() -> Result<TrackListWithAssociatedData, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_music_items;
    use playlist_core::models::track::load_linked_tracks;

    tracing::info!("Loading popular tracks");

    // Get all JD playlists
    let playlists = load_music_items::<PlayList>(bson::doc! {})
        .await
        .map_err(to_server_error)?;

    // Load all LinkedTrack documents
    let linked_tracks = load_linked_tracks(bson::doc! {})
        .await
        .map_err(to_server_error)?;

    let (sorted_track_ids, linked_track_sets) =
        aggregate_popular_tracks(&playlists, &linked_tracks);

    let track_data =
        get_track_list_with_associated_data(sorted_track_ids, None, Some(linked_track_sets))
            .await?;

    Ok(track_data)
}

/// Pure aggregation logic for the popular-tracks endpoint.
///
/// Counts how many playlists each track appears in (repeated track ids within a single
/// playlist count once), merges the counts of linked track versions into the most
/// popular version, sorts by count descending and truncates to the top 100.
///
/// Both sorts break count ties by track id, so the output — including which linked
/// version survives the merge when versions are tied — is fully deterministic
/// regardless of input (or `HashMap` iteration) order.
///
/// Returns the sorted track ids together with the linked-track group for each id (a
/// singleton set for tracks without linked versions).
#[cfg(feature = "server")]
fn aggregate_popular_tracks(
    playlists: &[PlayList],
    linked_tracks: &[LinkedTrack],
) -> (Vec<String>, Vec<HashSet<String>>) {
    // Create a map from track id to LinkedTrack, for all tracks that appear in linked tracks
    let mut linked_tracks_map: HashMap<String, &LinkedTrack> = HashMap::new();
    for linked_track in linked_tracks {
        for track_id in &linked_track.track_ids {
            // Assert that we don't have duplicate linked track entries
            if linked_tracks_map.contains_key(track_id) {
                tracing::warn!(
                    "Duplicate linked track entry for track ID {} in linked track ID {}",
                    track_id,
                    linked_track.id
                );
            }
            linked_tracks_map.insert(track_id.clone(), linked_track);
        }
    }

    // Count occurrences of each track across all playlists
    let mut track_count_map: HashMap<String, usize> = HashMap::new();
    for playlist in playlists {
        // Don't count duplicate track IDs within the same playlist
        let unique_track_ids: HashSet<String> = playlist.track_ids.iter().cloned().collect();
        for track_id in unique_track_ids {
            *track_count_map.entry(track_id).or_insert(0) += 1;
        }
    }

    // Sort all tracks by popularity ascending, ignoring linked versions for now.
    // This allows using the most popular version as the "main" version later when including
    // linked tracks (it is visited last, so it absorbs the group's counts). Ties are broken
    // by track id so the surviving version is deterministic.
    let mut all_track_count_sorted: Vec<(String, usize)> =
        track_count_map.clone().into_iter().collect();
    all_track_count_sorted.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

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

    // Now get the top 100 tracks, including the counts from linked versions.
    // Sort by count descending, breaking count ties by track id ascending.
    let mut track_count_sorted: Vec<(String, usize)> = track_count_map.into_iter().collect();
    track_count_sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    track_count_sorted.truncate(100);
    let sorted_track_ids = track_count_sorted
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<Vec<String>>();

    let linked_track_sets = sorted_track_ids
        .iter()
        .map(|track_id| {
            linked_tracks_map
                .get(track_id)
                .map(|lt| lt.track_ids.iter().cloned().collect())
                .unwrap_or_else(|| HashSet::from([track_id.clone()]))
        })
        .collect();

    (sorted_track_ids, linked_track_sets)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub tracks: TrackListWithAssociatedData,
    pub artists: Vec<Artist>,
    pub compiler_ids: Vec<String>,
    pub playlist_ids: Vec<String>,
}

/// Search music items
#[get("/api/search?search_terms")]
pub async fn do_search(search_terms: String) -> Result<SearchResults, ServerFnError> {
    use playlist_core::{generate_double_metaphone_codes, generate_n_grams, normalise_name};

    let search_terms_normalised = normalise_name(&search_terms);
    let search_double_metaphone_codes = generate_double_metaphone_codes(&search_terms);
    let search_n_grams = generate_n_grams(&search_terms_normalised);

    tracing::info!(
        "Search terms: '{}', normalised: '{}', n-grams: {:?}, double metaphone codes: {:?}",
        search_terms,
        search_terms_normalised,
        search_n_grams,
        search_double_metaphone_codes
    );

    let tracks = search_music_items::<Track>(
        &search_terms_normalised,
        &search_double_metaphone_codes,
        &search_n_grams,
    )
    .await?;

    let sorted_track_ids = tracks
        .iter()
        .map(|track| track.id.clone())
        .collect::<Vec<String>>();

    let tracks = get_track_list_with_associated_data(
        sorted_track_ids,
        Some(tracks.iter().map(|track| track.clone()).collect()),
        None,
    )
    .await?;

    let artists = search_music_items::<Artist>(
        &search_terms_normalised,
        &search_double_metaphone_codes,
        &search_n_grams,
    )
    .await?;

    let compiler_ids: Vec<String> = search_music_items::<Compiler>(
        &search_terms_normalised,
        &search_double_metaphone_codes,
        &search_n_grams,
    )
    .await?
    .iter()
    .map(|compiler| compiler.id.clone())
    .collect();

    let playlist_ids: Vec<String> = search_music_items::<PlayList>(
        &search_terms_normalised,
        &search_double_metaphone_codes,
        &search_n_grams,
    )
    .await?
    .iter()
    .map(|playlist| playlist.id.clone())
    .collect();

    Ok(SearchResults {
        tracks,
        artists,
        compiler_ids,
        playlist_ids,
    })
}

#[cfg(feature = "server")]
async fn search_music_items<T>(
    search_terms_normalised: &str,
    search_double_metaphone_codes: &[String],
    search_n_grams: &[String],
) -> Result<Vec<T>, ServerFnError>
where
    T: playlist_core::models::MusicItem + Send + Sync + Unpin + for<'de> serde::Deserialize<'de>,
{
    use fuzzt::algorithms::normalized_damerau_levenshtein;
    use mongodb::bson;
    use playlist_core::models::server::load_music_items;

    let items: Vec<T> = load_music_items::<T>(bson::doc! {
        "$or": [
            { "search_double_metaphone_codes": { "$in": search_double_metaphone_codes } },
            { "search_n_grams": { "$in": search_n_grams } },
        ]
    })
    .await
    .map_err(to_server_error)?;

    tracing::info!(
        "Found {} candidate items for search terms '{}'",
        items.len(),
        search_terms_normalised
    );

    let search_terms_unique: HashSet<&str> = search_terms_normalised.split_whitespace().collect();

    let mut item_scores = items
        .iter()
        .map(move |item| {
            // Find the best score for each search term against any of this item's terms
            let terms_score = search_terms_unique
                .iter()
                .map(|search_term| {
                    item.search_terms()
                        .iter()
                        .map(|item_term| normalized_damerau_levenshtein(search_term, item_term))
                        .fold(0.0, |a: f64, b: f64| a.max(b))
                })
                .sum::<f64>()
                / (search_terms_unique.len() as f64);

            let wholename_score =
                normalized_damerau_levenshtein(search_terms_normalised, item.name_normalised());

            let score: usize = (terms_score * 80.0 + wholename_score * 20.0) as usize;

            (item.clone(), score)
        })
        .collect::<Vec<(T, usize)>>();

    item_scores.sort_by(|a, b| b.1.cmp(&a.1));
    item_scores.truncate(50);
    item_scores = item_scores
        .into_iter()
        .filter(|(_item, score)| *score > 50)
        .collect();

    // let search_terms_unique: HashSet<&str> = search_terms_normalised.split_whitespace().collect();
    // item_scores.clone().iter().for_each(|(item, score)| {
    //     let item_terms_unique = item.search_terms();

    //     tracing::info!("item_terms_unique: '{:?}'", item_terms_unique);

    //     // Find the best score for each search term against any of this item's terms
    //     search_terms_unique
    //         .iter()
    //         .map(|search_term| {
    //             let item_terms_scores = item_terms_unique
    //                 .iter()
    //                 .map(|item_term| normalized_damerau_levenshtein(search_term, item_term))
    //                 .collect::<Vec<f64>>();
    //             let best_score = item_terms_scores
    //                 .iter()
    //                 .fold(0.0, |a: f64, b: &f64| a.max(*b));
    //             tracing::info!(
    //                 "  search_term: '{}', item_terms_scores: '{:?}', best_score: {}",
    //                 search_term,
    //                 item_terms_scores,
    //                 best_score
    //             );
    //         })
    //         .for_each(drop);

    //     let wholename_score =
    //         normalized_damerau_levenshtein(search_terms_normalised, item.name_normalised());
    //     tracing::info!("  wholename_score: {}", wholename_score);
    //     tracing::info!("  total score: {}", score);
    // });

    let result = item_scores.into_iter().map(|(item, _)| item).collect();

    Ok(result)
}

#[cfg(feature = "server")]
pub async fn get_track_list_with_associated_data(
    sorted_track_ids: Vec<String>,
    tracks: Option<Vec<Track>>,
    linked_tracks: Option<Vec<HashSet<String>>>,
) -> Result<TrackListWithAssociatedData, ServerFnError> {
    use mongodb::bson;
    use playlist_core::models::server::load_music_items;
    use playlist_core::models::track::load_linked_tracks;

    let tracks = match tracks {
        Some(t) => {
            let track_ids_set: HashSet<String> = sorted_track_ids.iter().cloned().collect();
            // Only include tracks that are in sorted_track_ids
            t.into_iter()
                .filter(|track| track_ids_set.contains(&track.id))
                .collect()
        }
        None => load_music_items::<Track>(bson::doc! {
            "_id": {"$in": sorted_track_ids.clone()}
        })
        .await
        .map_err(to_server_error)?,
    };

    let tracks_by_id: HashMap<String, Track> = tracks
        .into_iter()
        .map(|track| (track.id.clone(), track))
        .collect();

    // Check that we found all tracks
    if tracks_by_id.len() != sorted_track_ids.len() {
        let found_ids: HashSet<String> = tracks_by_id.keys().cloned().collect();
        let missing_ids: Vec<String> = sorted_track_ids
            .iter()
            .filter(|id| !found_ids.contains(*id))
            .cloned()
            .collect();
        tracing::warn!("Some tracks not found: {:?}", missing_ids);
    }

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
        load_music_items::<Artist>(bson::doc! {
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
        load_music_items::<Album>(bson::doc! {
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

    let linked_tracks = match linked_tracks {
        Some(linked_tracks_map) => linked_tracks_map,
        None => {
            let linked_tracks_docs = load_linked_tracks(bson::doc! {
                "track_ids": {"$in": &sorted_track_ids}
            })
            .await
            .map_err(to_server_error)?;

            let mut linked_tracks_map: HashMap<String, &LinkedTrack> = HashMap::new();
            for linked_track in &linked_tracks_docs {
                for track_id in &linked_track.track_ids {
                    linked_tracks_map.insert(track_id.clone(), linked_track);
                }
            }

            sorted_track_ids
                .iter()
                .map(|track_id| {
                    linked_tracks_map
                        .get(track_id)
                        .map(|lt| lt.track_ids.iter().cloned().collect())
                        .unwrap_or_else(|| HashSet::from([track_id.clone()]))
                })
                .collect()
        }
    };

    Ok(TrackListWithAssociatedData {
        sorted_track_ids,
        tracks_by_id,
        linked_tracks,
        artists_by_id,
        albums_by_id,
    })
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    fn playlist(id: &str, track_ids: &[&str]) -> PlayList {
        PlayList {
            id: id.to_string(),
            name: id.to_string(),
            name_normalised: id.to_string(),
            name_normalised_strong: id.to_string(),
            disambiguation: None,
            notes: None,
            data_maybe_missing: None,
            potential_duplicate: None,
            needs_review: None,
            external_service_associations: None,
            search_terms: vec![],
            search_double_metaphone_codes: vec![],
            search_n_grams: vec![],
            compiler_ids: vec![],
            track_ids: track_ids.iter().map(|s| s.to_string()).collect(),
            duration: 0.0,
            user_id: "user-test".to_string(),
            group_id: None,
            tag_ids: None,
            number: None,
            date: None,
        }
    }

    fn linked_track(id: &str, track_ids: &[&str]) -> LinkedTrack {
        LinkedTrack {
            id: id.to_string(),
            track_name_normalised_strong: id.to_string(),
            track_ids: track_ids.iter().map(|s| s.to_string()).collect(),
            artist_ids: vec![],
        }
    }

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn counts_playlist_occurrences_and_sorts_descending() {
        let playlists = vec![
            playlist("p1", &["track-a", "track-b"]),
            playlist("p2", &["track-a", "track-c"]),
            playlist("p3", &["track-a", "track-b"]),
        ];

        let (sorted_track_ids, linked_track_sets) = aggregate_popular_tracks(&playlists, &[]);

        // track-a: 3, track-b: 2, track-c: 1.
        assert_eq!(sorted_track_ids, vec!["track-a", "track-b", "track-c"]);
        // No linked versions: each track reports a singleton group of itself.
        assert_eq!(
            linked_track_sets,
            vec![set(&["track-a"]), set(&["track-b"]), set(&["track-c"])]
        );
    }

    #[test]
    fn duplicate_track_ids_within_a_playlist_count_once() {
        let playlists = vec![
            playlist("p1", &["track-a", "track-a", "track-a", "track-b"]),
            playlist("p2", &["track-b"]),
        ];

        let (sorted_track_ids, _) = aggregate_popular_tracks(&playlists, &[]);

        // track-a appears three times in p1 but counts once (1 total); track-b counts 2.
        assert_eq!(sorted_track_ids, vec!["track-b", "track-a"]);
    }

    #[test]
    fn linked_version_counts_merge_into_the_most_popular_version() {
        // v1 appears in two playlists, v2 in one; they are linked versions of one song.
        let playlists = vec![
            playlist("p1", &["track-v1", "track-x"]),
            playlist("p2", &["track-v1"]),
            playlist("p3", &["track-v2"]),
        ];
        let linked = vec![linked_track("linked-1", &["track-v1", "track-v2"])];

        let (sorted_track_ids, linked_track_sets) = aggregate_popular_tracks(&playlists, &linked);

        // v2's count folds into v1 (2 + 1 = 3 > track-x's 1); v2 itself disappears.
        assert_eq!(sorted_track_ids, vec!["track-v1", "track-x"]);
        // The surviving version reports its full linked group.
        assert_eq!(
            linked_track_sets,
            vec![set(&["track-v1", "track-v2"]), set(&["track-x"])]
        );
    }

    #[test]
    fn equal_counts_order_by_track_id() {
        let playlists = vec![
            playlist("p1", &["track-c", "track-a", "track-d"]),
            playlist("p2", &["track-d", "track-b"]),
        ];

        let (sorted_track_ids, _) = aggregate_popular_tracks(&playlists, &[]);

        // track-d has count 2; the count-1 tracks tie and order by id.
        assert_eq!(
            sorted_track_ids,
            vec!["track-d", "track-a", "track-b", "track-c"]
        );
    }

    #[test]
    fn tied_linked_merge_survivor_is_stable_across_input_orders() {
        // Both linked versions have count 1, so the merge survivor is decided purely by
        // the deterministic tie-break — not by input (or HashMap iteration) order.
        let scenario = |playlists: Vec<PlayList>, linked: Vec<LinkedTrack>| {
            aggregate_popular_tracks(&playlists, &linked)
        };

        let baseline = scenario(
            vec![
                playlist("p1", &["track-v1", "track-z"]),
                playlist("p2", &["track-v2"]),
            ],
            vec![linked_track("linked-1", &["track-v1", "track-v2"])],
        );

        // Same scenario with playlists and linked-group member order permuted.
        let permuted = scenario(
            vec![
                playlist("p2", &["track-v2"]),
                playlist("p1", &["track-z", "track-v1"]),
            ],
            vec![linked_track("linked-1", &["track-v2", "track-v1"])],
        );

        assert_eq!(baseline, permuted);

        // The merged group (count 2) outranks track-z (count 1), and exactly one
        // version of the pair survives, reporting the full linked group.
        let (sorted_track_ids, linked_track_sets) = baseline;
        assert_eq!(sorted_track_ids.len(), 2);
        assert_eq!(sorted_track_ids[1], "track-z");
        assert!(sorted_track_ids[0] == "track-v1" || sorted_track_ids[0] == "track-v2");
        assert_eq!(
            linked_track_sets,
            vec![set(&["track-v1", "track-v2"]), set(&["track-z"])]
        );
    }

    #[test]
    fn truncates_to_the_top_100_tracks() {
        // 105 count-1 tracks plus one count-2 track: the count-2 track leads, then the
        // count-1 tracks in id order, cut at 100 entries total.
        let count1_ids: Vec<String> = (0..105).map(|i| format!("track-{:03}", i)).collect();
        let mut p1_track_ids: Vec<&str> = count1_ids.iter().map(|s| s.as_str()).collect();
        p1_track_ids.push("track-top");
        let playlists = vec![
            playlist("p1", &p1_track_ids),
            playlist("p2", &["track-top"]),
        ];

        let (sorted_track_ids, linked_track_sets) = aggregate_popular_tracks(&playlists, &[]);

        assert_eq!(sorted_track_ids.len(), 100);
        assert_eq!(linked_track_sets.len(), 100);
        assert_eq!(sorted_track_ids[0], "track-top");
        assert_eq!(sorted_track_ids[1], "track-000");
        assert_eq!(sorted_track_ids[99], "track-098");
        assert!(!sorted_track_ids.contains(&"track-099".to_string()));
    }
}
