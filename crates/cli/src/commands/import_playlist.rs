use mongodb::Database;
use mongodb::bson::{Document, doc};
use playlist_core::database::generate_id;
use playlist_core::models::{
    ExternalServiceAssociation, ImageUrls, MusicItemBase, album::Album, artist::Artist,
    compiler::Compiler, playlist::PlayList, track::Track,
};
use playlist_core::{
    generate_double_metaphone_codes, generate_n_grams, normalise_name, normalise_name_strong,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use url_parse::core::Parser;

const SPOTIFY_API: &str = "https://api.spotify.com/v1";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
/// Client-credentials tokens have no associated user country, so Spotify requires an
/// explicit `market` to return track data (without it, Get Playlist omits `tracks` and
/// Get Playlist Items returns an error). Overridable via the SPOTIFY_MARKET env var.
const DEFAULT_MARKET: &str = "AU";

// --- Spotify Web API response shapes (only the fields we use; lenient on the rest). ---

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct SpImage {
    url: String,
}

#[derive(Deserialize)]
struct SpArtist {
    id: Option<String>,
    name: String,
}

#[derive(Deserialize)]
struct SpAlbum {
    id: Option<String>,
    name: String,
    #[serde(default)]
    images: Vec<SpImage>,
    #[serde(default)]
    artists: Vec<SpArtist>,
}

#[derive(Deserialize)]
struct SpTrack {
    id: Option<String>,
    name: String,
    #[serde(default)]
    artists: Vec<SpArtist>,
    album: Option<SpAlbum>,
    #[serde(default)]
    duration_ms: u64,
}

#[derive(Deserialize)]
struct SpPlaylistItem {
    track: Option<SpTrack>,
}

#[derive(Deserialize)]
struct SpTracksPage {
    #[serde(default)]
    items: Vec<SpPlaylistItem>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct SpOwner {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
}

/// Spotify user profile (Get User's Profile). Used to enrich the playlist owner we import
/// as a Compiler with a display name and profile images.
#[derive(Deserialize)]
struct SpUser {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    images: Vec<SpImage>,
}

#[derive(Deserialize)]
struct SpPlaylist {
    id: String,
    name: String,
    description: Option<String>,
    #[serde(default)]
    images: Vec<SpImage>,
    owner: SpOwner,
}

pub async fn import_playlist(
    database: Database,
    uri: &str,
    user_id: Option<String>,
    name_override: Option<String>,
    date: Option<f64>,
) -> Result<(), Box<dyn std::error::Error>> {
    if uri.contains("spotify.com") {
        do_spotify_import(database, uri, user_id, name_override, date).await?;
    } else {
        eprintln!("Unsupported service URI.");
        std::process::exit(1);
    }
    Ok(())
}

async fn do_spotify_import(
    database: Database,
    uri: &str,
    user_id_override: Option<String>,
    name_override: Option<String>,
    date: Option<f64>,
) -> Result<(), Box<dyn std::error::Error>> {
    const PARSE_ERROR_MSG: &str = "Not a valid Spotify playlist URL";
    let parsed = Parser::new(None).parse(uri)?;
    let segments = parsed.path.ok_or(PARSE_ERROR_MSG)?;
    if segments.len() < 2 {
        return Err(PARSE_ERROR_MSG.into());
    }
    let last_two_segments = &segments[segments.len() - 2..];
    if last_two_segments[0] != "playlist" {
        return Err(PARSE_ERROR_MSG.into());
    }
    let playlist_id = last_two_segments[1].clone();

    println!("Importing Spotify playlist with ID: {}... ", playlist_id);

    let client_id = std::env::var("SPOTIFY_CLIENT_ID")
        .map_err(|_| "SPOTIFY_CLIENT_ID environment variable is not set")?;
    let client_secret = std::env::var("SPOTIFY_CLIENT_SECRET")
        .map_err(|_| "SPOTIFY_CLIENT_SECRET environment variable is not set")?;
    let market = std::env::var("SPOTIFY_MARKET").unwrap_or_else(|_| DEFAULT_MARKET.to_string());

    let http = reqwest::Client::new();

    // Client Credentials flow: app-only access, sufficient for reading public playlists.
    let token = get_access_token(&http, &client_id, &client_secret).await?;

    // Playlist metadata. Note: Spotify-owned editorial/algorithmic playlists are no longer
    // accessible to apps in development mode and will return a 404 here; regular
    // user-created public playlists work fine.
    let playlist: SpPlaylist = api_get(
        &http,
        &token,
        &format!("{SPOTIFY_API}/playlists/{playlist_id}?market={market}"),
    )
    .await?;

    // Fetch all tracks via the dedicated, paginated endpoint.
    let mut items: Vec<SpPlaylistItem> = Vec::new();
    let mut offset = 0u32;
    loop {
        let page: SpTracksPage = api_get(
            &http,
            &token,
            &format!(
                "{SPOTIFY_API}/playlists/{playlist_id}/tracks?market={market}&limit=100&offset={offset}"
            ),
        )
        .await?;
        let fetched = page.items.len() as u32;
        items.extend(page.items);
        if fetched == 0 || page.next.is_none() {
            break;
        }
        offset += fetched;
    }

    println!(
        "Fetched playlist \"{}\" (owner: {}, {} items)",
        playlist.name,
        playlist.owner.id,
        items.len()
    );

    // Upsert every track (and its artists/album) and gather the resulting ids.
    let mut track_ids: Vec<String> = Vec::new();
    let mut duration_seconds: f64 = 0.0;
    for item in &items {
        let Some(track) = &item.track else { continue };
        let Some(track_spotify_id) = track.id.as_deref() else {
            // Skip local files / items without a Spotify id (can't be deduplicated).
            continue;
        };

        let track_artists = artist_refs(&track.artists);

        let album_id = match &track.album {
            Some(album) => Some(
                upsert_album(
                    &database,
                    album.id.as_deref(),
                    &album.name,
                    images_to_image_urls(&album.images),
                    &artist_refs(&album.artists),
                )
                .await?,
            ),
            None => None,
        };

        let artist_ids = upsert_artists(&database, &track_artists).await?;

        let duration_secs = track.duration_ms as f64 / 1000.0;
        let track_id = upsert_track(
            &database,
            track_spotify_id,
            &track.name,
            duration_secs,
            artist_ids,
            album_id,
        )
        .await?;

        duration_seconds += duration_secs;
        track_ids.push(track_id);
    }

    // Import the playlist's author(s) as Compilers and link them to the playlist.
    let compiler_ids = upsert_compilers(&http, &token, &database, &playlist.owner).await?;

    // Resolve the app-level user_id: CLI argument, then the IMPORT_USER_ID env var,
    // falling back to the Spotify owner id.
    let user_id = user_id_override
        .or_else(|| std::env::var("IMPORT_USER_ID").ok())
        .unwrap_or_else(|| playlist.owner.id.clone());

    let id = find_existing_id(&database, PlayList::collection_name(), &playlist.id)
        .await?
        .unwrap_or_else(generate_id);

    // The --name argument overrides the playlist's name as it is on the service.
    let name = name_override.unwrap_or_else(|| playlist.name.clone());

    let (search_terms, search_double_metaphone_codes, search_n_grams) =
        build_search_fields(&[normalise_name(&name)]);

    let playlist_doc = PlayList {
        id,
        name: name.clone(),
        name_normalised: normalise_name(&name),
        name_normalised_strong: normalise_name_strong(&name),
        disambiguation: None,
        notes: playlist.description.clone().filter(|d| !d.is_empty()),
        data_maybe_missing: None,
        potential_duplicate: None,
        needs_review: None,
        external_service_associations: Some(vec![ExternalServiceAssociation::Spotify {
            id: playlist.id.clone(),
            image_urls: images_to_image_urls(&playlist.images),
        }]),
        search_terms,
        search_double_metaphone_codes,
        search_n_grams,
        compiler_ids,
        track_ids,
        duration: duration_seconds,
        user_id,
        group_id: None,
        tag_ids: None,
        number: None,
        date,
    };

    upsert(&database, &playlist_doc).await?;

    println!(
        "Imported playlist \"{}\" with {} tracks.",
        playlist_doc.name,
        playlist_doc.track_ids.len()
    );

    Ok(())
}

/// Exchange client credentials for an app-only access token.
async fn get_access_token(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = http
        .post(TOKEN_URL)
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json::<TokenResponse>().await?.access_token)
}

/// Perform an authenticated GET and deserialize the JSON body, surfacing the response
/// body on non-success statuses (Spotify returns useful error detail there).
async fn api_get<T: DeserializeOwned>(
    http: &reqwest::Client,
    token: &str,
    url: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    let response = http.get(url).bearer_auth(token).send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Spotify API returned {} for {}: {}", status, url, body).into());
    }
    Ok(response.json::<T>().await?)
}

/// Collect (Spotify id, name) pairs for artists that have a Spotify id.
fn artist_refs(artists: &[SpArtist]) -> Vec<(String, String)> {
    artists
        .iter()
        .filter_map(|a| a.id.as_ref().map(|id| (id.clone(), a.name.clone())))
        .collect()
}

/// Upsert a track, returning its id. Artists and album must already be upserted; their
/// ids are passed in.
async fn upsert_track(
    database: &Database,
    spotify_id: &str,
    name: &str,
    duration_secs: f64,
    artist_ids: Vec<String>,
    album_id: Option<String>,
) -> Result<String, Box<dyn std::error::Error>> {
    let id = find_existing_id(database, Track::collection_name(), spotify_id)
        .await?
        .unwrap_or_else(generate_id);

    // Track search terms key primarily on the track name, mirroring the migration.
    let (search_terms, search_double_metaphone_codes, search_n_grams) =
        build_search_fields(&[normalise_name(name)]);

    let track_doc = Track {
        id: id.clone(),
        name: name.to_string(),
        name_normalised: normalise_name(name),
        name_normalised_strong: normalise_name_strong(name),
        disambiguation: None,
        notes: None,
        data_maybe_missing: None,
        potential_duplicate: None,
        needs_review: None,
        external_service_associations: Some(vec![ExternalServiceAssociation::Spotify {
            id: spotify_id.to_string(),
            image_urls: None,
        }]),
        search_terms,
        search_double_metaphone_codes,
        search_n_grams,
        artist_ids,
        album_id,
        duration: Some(duration_secs),
    };

    upsert(database, &track_doc).await?;
    Ok(id)
}

/// Upsert an album (and its artists), returning the album's id. Albums without a Spotify
/// id get a fresh, non-deduplicated id.
async fn upsert_album(
    database: &Database,
    spotify_id: Option<&str>,
    name: &str,
    image_urls: Option<ImageUrls>,
    artists: &[(String, String)],
) -> Result<String, Box<dyn std::error::Error>> {
    let artist_ids = upsert_artists(database, artists).await?;

    let id = match spotify_id {
        Some(sid) => find_existing_id(database, Album::collection_name(), sid)
            .await?
            .unwrap_or_else(generate_id),
        None => generate_id(),
    };

    let (search_terms, search_double_metaphone_codes, search_n_grams) =
        build_search_fields(&[normalise_name(name)]);

    let album_doc = Album {
        id: id.clone(),
        name: name.to_string(),
        name_normalised: normalise_name(name),
        name_normalised_strong: normalise_name_strong(name),
        disambiguation: None,
        notes: None,
        data_maybe_missing: None,
        potential_duplicate: None,
        needs_review: None,
        external_service_associations: spotify_id.map(|sid| {
            vec![ExternalServiceAssociation::Spotify {
                id: sid.to_string(),
                image_urls,
            }]
        }),
        search_terms,
        search_double_metaphone_codes,
        search_n_grams,
        artist_ids,
    };

    upsert(database, &album_doc).await?;
    Ok(id)
}

/// Upsert a list of artists, returning the resulting ids.
async fn upsert_artists(
    database: &Database,
    artists: &[(String, String)],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut ids = Vec::new();
    for (spotify_id, name) in artists {
        let id = find_existing_id(database, Artist::collection_name(), spotify_id)
            .await?
            .unwrap_or_else(generate_id);

        let (search_terms, search_double_metaphone_codes, search_n_grams) =
            build_search_fields(&[normalise_name(name)]);

        let artist_doc = Artist {
            id: id.clone(),
            name: name.to_string(),
            name_normalised: normalise_name(name),
            name_normalised_strong: normalise_name_strong(name),
            disambiguation: None,
            notes: None,
            data_maybe_missing: None,
            potential_duplicate: None,
            needs_review: None,
            external_service_associations: Some(vec![ExternalServiceAssociation::Spotify {
                id: spotify_id.clone(),
                image_urls: None,
            }]),
            search_terms,
            search_double_metaphone_codes,
            search_n_grams,
            alt_names: None,
        };

        upsert(database, &artist_doc).await?;
        ids.push(id);
    }
    Ok(ids)
}

/// Upsert the playlist's author(s) as Compilers, returning their ids. Spotify playlists
/// have a single owner; we fetch the owner's user profile to enrich the Compiler with a
/// display name and profile images.
async fn upsert_compilers(
    http: &reqwest::Client,
    token: &str,
    database: &Database,
    owner: &SpOwner,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let profile: SpUser = api_get(http, token, &format!("{SPOTIFY_API}/users/{}", owner.id)).await?;

    // Prefer the profile display name, then the owner display name, then the id.
    let name = profile
        .display_name
        .filter(|n| !n.is_empty())
        .or_else(|| owner.display_name.clone().filter(|n| !n.is_empty()))
        .unwrap_or_else(|| owner.id.clone());

    let id = upsert_compiler(
        database,
        &owner.id,
        &name,
        images_to_image_urls(&profile.images),
    )
    .await?;

    Ok(vec![id])
}

/// Upsert a single Compiler keyed on its Spotify user id, returning its id.
async fn upsert_compiler(
    database: &Database,
    spotify_id: &str,
    name: &str,
    image_urls: Option<ImageUrls>,
) -> Result<String, Box<dyn std::error::Error>> {
    let id = find_existing_id(database, Compiler::collection_name(), spotify_id)
        .await?
        .unwrap_or_else(generate_id);

    let (search_terms, search_double_metaphone_codes, search_n_grams) =
        build_search_fields(&[normalise_name(name)]);

    let compiler_doc = Compiler {
        id: id.clone(),
        name: name.to_string(),
        name_normalised: normalise_name(name),
        name_normalised_strong: normalise_name_strong(name),
        disambiguation: None,
        notes: None,
        data_maybe_missing: None,
        potential_duplicate: None,
        needs_review: None,
        external_service_associations: Some(vec![ExternalServiceAssociation::Spotify {
            id: spotify_id.to_string(),
            image_urls,
        }]),
        search_terms,
        search_double_metaphone_codes,
        search_n_grams,
    };

    upsert(database, &compiler_doc).await?;
    Ok(id)
}

/// Look up an existing music item by its Spotify id, returning its `_id` if present.
async fn find_existing_id(
    database: &Database,
    collection_name: &str,
    spotify_id: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let collection = database.collection::<Document>(collection_name);
    let existing = collection
        .find_one(doc! { "external_service_associations.Spotify.id": spotify_id })
        .await?;
    Ok(existing.and_then(|doc| doc.get_str("_id").ok().map(|s| s.to_string())))
}

/// Insert or replace a music item, keyed on its `_id`.
async fn upsert<T>(database: &Database, item: &T) -> Result<(), Box<dyn std::error::Error>>
where
    T: MusicItemBase + serde::Serialize + DeserializeOwned + Send + Sync,
{
    let collection = database.collection::<T>(T::collection_name());
    collection
        .replace_one(doc! { "_id": item.id() }, item)
        .upsert(true)
        .await?;
    Ok(())
}

/// Build the (search_terms, double_metaphone_codes, n_grams) triple from a set of
/// already-normalised search strings, mirroring the migration's search indexing.
pub(crate) fn build_search_fields(
    normalised_search_strings: &[String],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    use std::collections::HashSet;
    let mut search_terms = HashSet::<String>::new();
    let mut double_metaphone_codes = HashSet::<String>::new();
    let mut n_grams = HashSet::<String>::new();

    for search_string in normalised_search_strings {
        for term in search_string.split_whitespace() {
            search_terms.insert(term.to_string());
            double_metaphone_codes.extend(generate_double_metaphone_codes(term));
            n_grams.extend(generate_n_grams(term));
        }
    }

    (
        search_terms.into_iter().collect(),
        double_metaphone_codes.into_iter().collect(),
        n_grams.into_iter().collect(),
    )
}

/// Map Spotify images (ordered largest-first) to the app's small/medium/large slots.
fn images_to_image_urls(images: &[SpImage]) -> Option<ImageUrls> {
    if images.is_empty() {
        return None;
    }
    let large = images.first().map(|i| i.url.clone());
    let small = images.last().map(|i| i.url.clone());
    let medium = if images.len() >= 3 {
        Some(images[images.len() / 2].url.clone())
    } else {
        None
    };
    Some(ImageUrls {
        small,
        medium,
        large,
    })
}

// https://play.spotify.com/user/1277013959/playlist/0j0RUMi4syrTPG6jtHPNux
// https://open.spotify.com/user/1255231405/playlist/1nanrCTj3UhGJanaIVPyi8
// https://open.spotify.com/playlist/3y1VsqLEvW4vUQvIAVjT1P
