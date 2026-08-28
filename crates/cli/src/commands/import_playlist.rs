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
use std::collections::HashMap;
use url_parse::core::Parser;

/// A music service the importer can key external associations on. It lets the shared
/// upsert helpers build the right [`ExternalServiceAssociation`] variant and query the
/// matching `external_service_associations.<Service>.id` field, so the same upsert and
/// idempotency logic serves every service.
#[derive(Debug, Clone, Copy)]
enum Service {
    Spotify,
    Tidal,
}

impl Service {
    /// The externally-tagged enum key, used as the BSON field name for this service's
    /// association (e.g. the `Spotify` in `external_service_associations.Spotify.id`).
    fn key(self) -> &'static str {
        match self {
            Service::Spotify => "Spotify",
            Service::Tidal => "Tidal",
        }
    }

    /// Build this service's association from an external id and optional images.
    fn association(self, id: String, image_urls: Option<ImageUrls>) -> ExternalServiceAssociation {
        match self {
            Service::Spotify => ExternalServiceAssociation::Spotify { id, image_urls },
            Service::Tidal => ExternalServiceAssociation::Tidal { id, image_urls },
        }
    }
}

const SPOTIFY_API: &str = "https://api.spotify.com/v1";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// Spotify Web API base URL. Overridable via the SPOTIFY_API_BASE env var (used by tests
/// to point the importer at a mock server).
fn spotify_api_base() -> String {
    std::env::var("SPOTIFY_API_BASE").unwrap_or_else(|_| SPOTIFY_API.to_string())
}

/// Spotify token endpoint URL. Overridable via the SPOTIFY_TOKEN_URL env var (used by
/// tests to point the importer at a mock server).
fn spotify_token_url() -> String {
    std::env::var("SPOTIFY_TOKEN_URL").unwrap_or_else(|_| TOKEN_URL.to_string())
}
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
    } else if uri.contains("tidal.com") {
        do_tidal_import(database, uri, user_id, name_override, date).await?;
    } else {
        return Err("Unsupported service URI.".into());
    }
    Ok(())
}

/// Extract the playlist id from a Spotify playlist URL. Accepts the various URL shapes
/// Spotify has used over the years (see the examples at the bottom of this file); the
/// playlist id is the segment following a final "playlist" path segment.
pub(crate) fn parse_spotify_playlist_id(uri: &str) -> Result<String, Box<dyn std::error::Error>> {
    parse_playlist_id(uri, "Not a valid Spotify playlist URL")
}

/// Extract the playlist id from a music-service playlist URL: the segment following a
/// final "playlist" path segment. Shared by the Spotify and Tidal URL parsers (their URL
/// shapes both end in `.../playlist/<id>`); `error_msg` names the service in failures.
fn parse_playlist_id(
    uri: &str,
    error_msg: &'static str,
) -> Result<String, Box<dyn std::error::Error>> {
    let parsed = Parser::new(None).parse(uri)?;
    let mut segments = parsed.path.ok_or(error_msg)?;
    // Tolerate a trailing slash (e.g. ".../playlist/<id>/"), which yields a trailing
    // empty segment, so a real share link ending in "/" is still accepted.
    if segments.last().is_some_and(|s| s.is_empty()) {
        segments.pop();
    }
    if segments.len() < 2 {
        return Err(error_msg.into());
    }
    let last_two_segments = &segments[segments.len() - 2..];
    if last_two_segments[0] != "playlist" {
        return Err(error_msg.into());
    }
    Ok(last_two_segments[1].clone())
}

async fn do_spotify_import(
    database: Database,
    uri: &str,
    user_id_override: Option<String>,
    name_override: Option<String>,
    date: Option<f64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let playlist_id = parse_spotify_playlist_id(uri)?;

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
    let api_base = spotify_api_base();

    let playlist: SpPlaylist = api_get(
        &http,
        &token,
        &format!("{api_base}/playlists/{playlist_id}?market={market}"),
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
                "{api_base}/playlists/{playlist_id}/tracks?market={market}&limit=100&offset={offset}"
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
                    Service::Spotify,
                    album.id.as_deref(),
                    &album.name,
                    images_to_image_urls(&album.images),
                    &artist_refs(&album.artists),
                )
                .await?,
            ),
            None => None,
        };

        let artist_ids = upsert_artists(&database, Service::Spotify, &track_artists).await?;

        let duration_secs = track.duration_ms as f64 / 1000.0;
        let track_id = upsert_track(
            &database,
            Service::Spotify,
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

    let id = find_existing_id(
        &database,
        PlayList::collection_name(),
        Service::Spotify,
        &playlist.id,
    )
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
        .post(spotify_token_url())
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
    service: Service,
    external_id: &str,
    name: &str,
    duration_secs: f64,
    artist_ids: Vec<String>,
    album_id: Option<String>,
) -> Result<String, Box<dyn std::error::Error>> {
    let id = find_existing_id(database, Track::collection_name(), service, external_id)
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
        external_service_associations: Some(vec![
            service.association(external_id.to_string(), None),
        ]),
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
    service: Service,
    external_id: Option<&str>,
    name: &str,
    image_urls: Option<ImageUrls>,
    artists: &[(String, String)],
) -> Result<String, Box<dyn std::error::Error>> {
    let artist_ids = upsert_artists(database, service, artists).await?;

    let id = match external_id {
        Some(sid) => find_existing_id(database, Album::collection_name(), service, sid)
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
        external_service_associations: external_id
            .map(|sid| vec![service.association(sid.to_string(), image_urls)]),
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
    service: Service,
    artists: &[(String, String)],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut ids = Vec::new();
    for (external_id, name) in artists {
        let id = find_existing_id(database, Artist::collection_name(), service, external_id)
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
            external_service_associations: Some(vec![
                service.association(external_id.clone(), None),
            ]),
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
    let profile: SpUser = api_get(
        http,
        token,
        &format!("{}/users/{}", spotify_api_base(), owner.id),
    )
    .await?;

    // Prefer the profile display name, then the owner display name, then the id.
    let name = profile
        .display_name
        .filter(|n| !n.is_empty())
        .or_else(|| owner.display_name.clone().filter(|n| !n.is_empty()))
        .unwrap_or_else(|| owner.id.clone());

    let id = upsert_compiler(
        database,
        Service::Spotify,
        &owner.id,
        &name,
        images_to_image_urls(&profile.images),
    )
    .await?;

    Ok(vec![id])
}

/// Upsert a single Compiler keyed on its external service user id, returning its id.
async fn upsert_compiler(
    database: &Database,
    service: Service,
    external_id: &str,
    name: &str,
    image_urls: Option<ImageUrls>,
) -> Result<String, Box<dyn std::error::Error>> {
    let id = find_existing_id(database, Compiler::collection_name(), service, external_id)
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
        external_service_associations: Some(vec![
            service.association(external_id.to_string(), image_urls),
        ]),
        search_terms,
        search_double_metaphone_codes,
        search_n_grams,
    };

    upsert(database, &compiler_doc).await?;
    Ok(id)
}

/// Look up an existing music item by its external service id, returning its `_id` if present.
async fn find_existing_id(
    database: &Database,
    collection_name: &str,
    service: Service,
    external_id: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let collection = database.collection::<Document>(collection_name);
    let mut filter = Document::new();
    filter.insert(
        format!("external_service_associations.{}.id", service.key()),
        external_id,
    );
    let existing = collection.find_one(filter).await?;
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

// ---------------------------------------------------------------------------
// Tidal
// ---------------------------------------------------------------------------
//
// Tidal's v2 Application Programming Interface (API) is a JavaScript Object Notation
// (JSON):API service: responses are compound documents with a `data` object/array, an
// `included` array of related resources, and `links` for pagination. Related resources (a
// track's artists and album, a playlist's cover art and owner) are referenced by
// (type, id) identifiers that are resolved against `included`. Access uses the Open
// Authorization 2.0 (OAuth2) client-credentials (app-only) flow, which — like the Spotify
// importer — can read public playlists but not a user's private ones.

const TIDAL_API: &str = "https://openapi.tidal.com/v2";
const TIDAL_TOKEN_URL: &str = "https://auth.tidal.com/v1/oauth2/token";
/// Catalogue availability is country-scoped, so every catalogue call sends a
/// `countryCode`. Overridable via the TIDAL_COUNTRY env var.
const TIDAL_DEFAULT_COUNTRY: &str = "AU";
/// The JSON:API media type Tidal requires; omitting it as the `Accept` header yields a
/// 406 response.
const TIDAL_MEDIA_TYPE: &str = "application/vnd.api+json";
/// Tidal rate-limits aggressively; bound how many times a single request is retried after
/// a 429 before giving up.
const TIDAL_MAX_RETRIES: u32 = 5;

/// Tidal v2 API base URL. Overridable via the TIDAL_API_BASE env var (used by tests to
/// point the importer at a mock server).
fn tidal_api_base() -> String {
    std::env::var("TIDAL_API_BASE").unwrap_or_else(|_| TIDAL_API.to_string())
}

/// Tidal token endpoint URL. Overridable via the TIDAL_TOKEN_URL env var (used by tests
/// to point the importer at a mock server).
fn tidal_token_url() -> String {
    std::env::var("TIDAL_TOKEN_URL").unwrap_or_else(|_| TIDAL_TOKEN_URL.to_string())
}

/// The country code sent as `countryCode` on catalogue calls. Overridable via the
/// TIDAL_COUNTRY env var.
fn tidal_country() -> String {
    std::env::var("TIDAL_COUNTRY").unwrap_or_else(|_| TIDAL_DEFAULT_COUNTRY.to_string())
}

// --- Tidal JSON:API response shapes (only the fields we use; lenient on the rest). ---

#[derive(Deserialize)]
struct TidalTokenResponse {
    access_token: String,
}

/// A JSON:API resource: an identifier (`type` + `id`) that, when the resource is a full
/// object in `data`/`included`, also carries `attributes` and `relationships`. Both are
/// kept as raw JSON so one type covers every resource kind (playlists, tracks, artists,
/// albums, artworks, users) a compound document mixes together.
#[derive(Deserialize)]
struct TidalResource {
    #[serde(rename = "type")]
    kind: String,
    id: String,
    #[serde(default)]
    attributes: serde_json::Value,
    #[serde(default)]
    relationships: serde_json::Value,
}

/// A single-resource document (e.g. GET /playlists/{id}).
#[derive(Deserialize)]
struct TidalSingleDoc {
    data: TidalResource,
    #[serde(default)]
    included: Vec<TidalResource>,
}

/// A relationship/collection document (e.g. the paginated playlist-items endpoint).
#[derive(Deserialize)]
struct TidalMultiDoc {
    #[serde(default)]
    data: Vec<TidalResource>,
    #[serde(default)]
    included: Vec<TidalResource>,
    #[serde(default)]
    links: TidalLinks,
}

#[derive(Deserialize, Default)]
struct TidalLinks {
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    meta: TidalLinksMeta,
}

#[derive(Deserialize, Default)]
struct TidalLinksMeta {
    #[serde(default, rename = "nextCursor")]
    next_cursor: Option<String>,
}

/// One image file of an artwork. Tidal returns a complete `href` URL (no templating) plus
/// the pixel dimensions.
#[derive(Deserialize)]
struct TidalArtworkFile {
    href: String,
    #[serde(default)]
    meta: TidalArtworkFileMeta,
}

#[derive(Deserialize, Default)]
struct TidalArtworkFileMeta {
    #[serde(default)]
    width: u32,
}

/// Read a string attribute from a resource's `attributes` object.
fn tidal_attr_str<'a>(res: &'a TidalResource, key: &str) -> Option<&'a str> {
    res.attributes.get(key).and_then(|v| v.as_str())
}

/// Collect the resource ids referenced by a to-many relationship, handling both the array
/// form (`data: [...]`) and, defensively, a single-object form (`data: {...}`).
fn tidal_rel_ids(res: &TidalResource, key: &str) -> Vec<String> {
    let Some(data) = res.relationships.get(key).and_then(|r| r.get("data")) else {
        return Vec::new();
    };
    match data {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.get("id").and_then(|v| v.as_str()).map(str::to_string))
            .collect(),
        serde_json::Value::Object(_) => data
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .into_iter()
            .flatten()
            .collect(),
        _ => Vec::new(),
    }
}

/// Look up a full resource by (type, id) in an index built over a document's `included`.
fn tidal_lookup<'a>(
    index: &HashMap<(String, String), &'a TidalResource>,
    kind: &str,
    id: &str,
) -> Option<&'a TidalResource> {
    index.get(&(kind.to_string(), id.to_string())).copied()
}

/// Map an artwork resource's image files to the app's small/medium/large slots. Tidal does
/// not guarantee an ordering, so the files are sorted by width (smallest -> small,
/// largest -> large, middle -> medium).
fn tidal_artwork_image_urls(artwork: &TidalResource) -> Option<ImageUrls> {
    let files_value = artwork.attributes.get("files")?;
    let files: Vec<TidalArtworkFile> = serde_json::from_value(files_value.clone()).ok()?;
    if files.is_empty() {
        return None;
    }
    let mut sorted: Vec<&TidalArtworkFile> = files.iter().collect();
    sorted.sort_by_key(|file| file.meta.width);
    let small = sorted.first().map(|file| file.href.clone());
    let large = sorted.last().map(|file| file.href.clone());
    let medium = if sorted.len() >= 3 {
        Some(sorted[sorted.len() / 2].href.clone())
    } else {
        None
    };
    Some(ImageUrls {
        small,
        medium,
        large,
    })
}

/// Collect (id, name) pairs for the artists a resource references, resolving each artist's
/// name from the included index; artists not embedded in `included` are skipped.
fn tidal_artist_refs(
    index: &HashMap<(String, String), &TidalResource>,
    res: &TidalResource,
) -> Vec<(String, String)> {
    tidal_rel_ids(res, "artists")
        .into_iter()
        .filter_map(|artist_id| {
            let name = tidal_lookup(index, "artists", &artist_id)
                .and_then(|artist| tidal_attr_str(artist, "name"))
                .filter(|name| !name.is_empty())?
                .to_string();
            Some((artist_id, name))
        })
        .collect()
}

/// The raw cursor for the next page, preferring the explicit `meta.nextCursor` and
/// otherwise parsing the `page[cursor]` query value out of the relative `links.next` URL.
/// The value is returned decoded (the `meta.nextCursor` form is already raw; the value
/// parsed out of `links.next` is percent-decoded) so the caller can re-encode it exactly
/// once when building the next request. Returns None when there is no next page.
fn tidal_next_cursor(links: &TidalLinks) -> Option<String> {
    if let Some(cursor) = links.meta.next_cursor.as_deref() {
        if !cursor.is_empty() {
            return Some(cursor.to_string());
        }
    }
    let next = links.next.as_deref()?;
    let marker = "page[cursor]=";
    let start = next.find(marker)? + marker.len();
    let rest = &next[start..];
    let end = rest.find('&').unwrap_or(rest.len());
    let cursor = &rest[..end];
    (!cursor.is_empty()).then(|| percent_decode(cursor))
}

/// Percent-decode a URL query value: `%XX` escapes become their byte, and `+` becomes a
/// space (application/x-www-form-urlencoded semantics). Invalid escapes are left verbatim.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 3 <= bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    None => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse an International Organization for Standardization (ISO) 8601 duration such as
/// "PT3M58S" or "PT1H2M3S" into seconds. Only the time component (after the `T`) is
/// interpreted, which is all Tidal uses for track durations. Returns None for a
/// malformed duration.
fn tidal_iso8601_duration_secs(value: &str) -> Option<f64> {
    let rest = value.trim().strip_prefix('P')?;
    let time_part = match rest.split_once('T') {
        Some((_date, time)) => time,
        // A date-only duration contributes no track seconds.
        None => return Some(0.0),
    };
    let mut seconds = 0.0_f64;
    let mut number = String::new();
    for c in time_part.chars() {
        if c.is_ascii_digit() || c == '.' {
            number.push(c);
        } else {
            let magnitude: f64 = number.parse().ok()?;
            number.clear();
            match c {
                'H' => seconds += magnitude * 3600.0,
                'M' => seconds += magnitude * 60.0,
                'S' => seconds += magnitude,
                _ => return None,
            }
        }
    }
    // Trailing digits with no unit designator make the duration malformed.
    if !number.is_empty() {
        return None;
    }
    Some(seconds)
}

/// Extract the playlist id (a Universally Unique Identifier, UUID) from a Tidal playlist
/// URL. Accepts the shapes Tidal uses (see the examples at the bottom of this section);
/// the id is the segment following a final "playlist" path segment.
pub(crate) fn parse_tidal_playlist_id(uri: &str) -> Result<String, Box<dyn std::error::Error>> {
    parse_playlist_id(uri, "Not a valid Tidal playlist URL")
}

/// Exchange client credentials for an app-only access token.
async fn get_tidal_access_token(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = http
        .post(tidal_token_url())
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json::<TidalTokenResponse>().await?.access_token)
}

/// Perform an authenticated JSON:API GET and deserialize the body. The `query` parameters
/// are appended and percent-encoded by the client (so cursors, commas in `include`, and
/// the brackets in `page[cursor]` are escaped correctly); the required `Accept` header is
/// sent; 429 responses are retried (honouring `Retry-After`); and other non-success
/// statuses surface the response body.
async fn tidal_api_get<T, Q>(
    http: &reqwest::Client,
    token: &str,
    url: &str,
    query: &Q,
) -> Result<T, Box<dyn std::error::Error>>
where
    T: DeserializeOwned,
    Q: serde::Serialize + ?Sized,
{
    let mut attempt = 0;
    loop {
        let response = http
            .get(url)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, TIDAL_MEDIA_TYPE)
            .query(query)
            .send()
            .await?;
        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < TIDAL_MAX_RETRIES {
            // Retry-After is in seconds; cap the wait so a hostile value cannot stall the
            // import indefinitely.
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(5)
                .min(60);
            attempt += 1;
            eprintln!(
                "Tidal API rate-limited (429); retrying in {retry_after}s (attempt {attempt}/{TIDAL_MAX_RETRIES})"
            );
            tokio::time::sleep(std::time::Duration::from_secs(retry_after)).await;
            continue;
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Tidal API returned {} for {}: {}", status, url, body).into());
        }

        return Ok(response.json::<T>().await?);
    }
}

/// The playlist owner (creator) as a (Tidal id, optional display name) pair, read from the
/// playlist document's embedded relationships. Prefers the `ownerProfiles` profile (the
/// only owner an app-only token can see; its `name` is often empty) and falls back to an
/// `owners` user (`username`). Returns None when neither relationship is populated. A
/// display name is not guaranteed under client credentials, so callers fall back to the id.
fn tidal_owner(playlist: &TidalSingleDoc) -> Option<(String, Option<String>)> {
    let owner_id = tidal_rel_ids(&playlist.data, "ownerProfiles")
        .into_iter()
        .next()
        .or_else(|| tidal_rel_ids(&playlist.data, "owners").into_iter().next())?;
    let name = playlist
        .included
        .iter()
        .find(|res| res.id == owner_id)
        .and_then(|res| tidal_attr_str(res, "name").or_else(|| tidal_attr_str(res, "username")))
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    Some((owner_id, name))
}

async fn do_tidal_import(
    database: Database,
    uri: &str,
    user_id_override: Option<String>,
    name_override: Option<String>,
    date: Option<f64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let playlist_id = parse_tidal_playlist_id(uri)?;

    println!("Importing Tidal playlist with ID: {}... ", playlist_id);

    let client_id = std::env::var("TIDAL_CLIENT_ID")
        .map_err(|_| "TIDAL_CLIENT_ID environment variable is not set")?;
    let client_secret = std::env::var("TIDAL_CLIENT_SECRET")
        .map_err(|_| "TIDAL_CLIENT_SECRET environment variable is not set")?;
    let country = tidal_country();

    let http = reqwest::Client::new();

    // Client Credentials flow: app-only access, sufficient for reading public playlists.
    let token = get_tidal_access_token(&http, &client_id, &client_secret).await?;

    let api_base = tidal_api_base();

    // Playlist metadata, its cover art, and its owner, all embedded in one call. The
    // owner comes via `ownerProfiles` (an `artists`-typed public profile) — that is the
    // only owner the app-only client-credentials token can see; the `owners` relationship
    // (a `users` resource) comes back empty app-only, and its `/relationships/owners`
    // sub-endpoint 404s. `owners` is still requested as a fallback for playlist types that
    // might populate it.
    let playlist: TidalSingleDoc = tidal_api_get(
        &http,
        &token,
        &format!("{api_base}/playlists/{playlist_id}"),
        &[
            ("countryCode", country.as_str()),
            ("include", "coverArt,ownerProfiles,owners"),
        ],
    )
    .await?;

    let playlist_name = tidal_attr_str(&playlist.data, "name")
        .unwrap_or_default()
        .to_string();
    let description = tidal_attr_str(&playlist.data, "description")
        .map(str::to_string)
        .filter(|d| !d.is_empty());

    // Cover art: the first coverArt artwork's files (looked up by id in `included`).
    let playlist_images = tidal_rel_ids(&playlist.data, "coverArt")
        .iter()
        .find_map(|artwork_id| playlist.included.iter().find(|res| &res.id == artwork_id))
        .and_then(tidal_artwork_image_urls);

    // Owner (creator) as a (Tidal id, optional display name) pair, best effort: prefer the
    // `ownerProfiles` profile, then the `owners` user. The profile's name is frequently
    // empty, so callers fall back to the id.
    let owner = tidal_owner(&playlist);

    // Fetch every playlist item (paginated), with each track's artists and album embedded
    // via a nested include, so one pass per page resolves the whole catalogue.
    let mut items: Vec<TidalResource> = Vec::new();
    let mut included: Vec<TidalResource> = Vec::new();
    let items_url = format!("{api_base}/playlists/{playlist_id}/relationships/items");
    let mut cursor: Option<String> = None;
    loop {
        // Owned query values so reassigning `cursor` below never conflicts with a borrow.
        let mut query: Vec<(&str, String)> = vec![
            ("countryCode", country.clone()),
            ("include", "items.artists,items.albums".to_string()),
        ];
        if let Some(cursor) = &cursor {
            query.push(("page[cursor]", cursor.clone()));
        }
        let mut page: TidalMultiDoc = tidal_api_get(&http, &token, &items_url, &query).await?;
        items.append(&mut page.data);
        included.append(&mut page.included);
        match tidal_next_cursor(&page.links) {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }

    // Index the accumulated `included` resources by (type, id) for relationship lookups.
    let index: HashMap<(String, String), &TidalResource> = included
        .iter()
        .map(|res| ((res.kind.clone(), res.id.clone()), res))
        .collect();

    // Upsert every track (and its artists/album) in playlist order, gathering ids.
    let mut track_ids: Vec<String> = Vec::new();
    let mut duration_seconds: f64 = 0.0;
    for item in &items {
        // Playlist items can be tracks or videos; only tracks are imported.
        if item.kind != "tracks" {
            continue;
        }
        let Some(track) = tidal_lookup(&index, "tracks", &item.id) else {
            // The track resource was not embedded (e.g. unavailable in this country).
            continue;
        };
        let Some(title) = tidal_attr_str(track, "title").filter(|title| !title.is_empty()) else {
            continue;
        };
        // Tidal splits a track name into a title and an optional version qualifier (e.g.
        // "Radio Edit"); combine them into a single display name.
        let name = match tidal_attr_str(track, "version").filter(|version| !version.is_empty()) {
            Some(version) => format!("{title} ({version})"),
            None => title.to_string(),
        };

        let duration_secs = tidal_attr_str(track, "duration")
            .and_then(tidal_iso8601_duration_secs)
            .unwrap_or(0.0);

        let track_artists = tidal_artist_refs(&index, track);
        let artist_ids = upsert_artists(&database, Service::Tidal, &track_artists).await?;

        // Resolve the track's album (its first album relationship with a title) from the
        // included resources. Albums without a usable title are skipped, mirroring the
        // track guard, so a blank-named album is never persisted.
        let mut album_id: Option<String> = None;
        for candidate_album_id in tidal_rel_ids(track, "albums") {
            let Some(album) = tidal_lookup(&index, "albums", &candidate_album_id) else {
                continue;
            };
            let Some(album_name) = tidal_attr_str(album, "title").filter(|title| !title.is_empty())
            else {
                continue;
            };
            let album_artists = tidal_artist_refs(&index, album);
            album_id = Some(
                upsert_album(
                    &database,
                    Service::Tidal,
                    Some(candidate_album_id.as_str()),
                    album_name,
                    None,
                    &album_artists,
                )
                .await?,
            );
            break;
        }

        let track_id = upsert_track(
            &database,
            Service::Tidal,
            &item.id,
            &name,
            duration_secs,
            artist_ids,
            album_id,
        )
        .await?;

        duration_seconds += duration_secs;
        track_ids.push(track_id);
    }

    // Distinguish a genuinely empty playlist from a systemic resolution failure (e.g. a
    // country in which nothing is available, or the related resources not being embedded).
    let track_item_count = items.iter().filter(|item| item.kind == "tracks").count();
    if track_ids.is_empty() && track_item_count > 0 {
        eprintln!(
            "Warning: the playlist has {track_item_count} track item(s) but none could be \
             resolved — check TIDAL_COUNTRY and that the tracks are available; importing an \
             empty playlist."
        );
    }

    println!(
        "Fetched playlist \"{}\" ({} items)",
        playlist_name,
        track_ids.len()
    );

    // Import the playlist's owner as a Compiler (best effort — app-only tokens may not
    // expose a display name; the owner id is the stable key either way).
    let mut compiler_ids: Vec<String> = Vec::new();
    if let Some((owner_id, owner_name)) = &owner {
        let name = owner_name.clone().unwrap_or_else(|| owner_id.clone());
        let compiler_id = upsert_compiler(&database, Service::Tidal, owner_id, &name, None).await?;
        compiler_ids.push(compiler_id);
    }

    // Resolve the app-level user_id: CLI argument, then the IMPORT_USER_ID env var, then
    // the Tidal owner id, finally the playlist id.
    let user_id = user_id_override
        .or_else(|| std::env::var("IMPORT_USER_ID").ok())
        .or_else(|| owner.as_ref().map(|(owner_id, _)| owner_id.clone()))
        .unwrap_or_else(|| playlist_id.clone());

    let id = find_existing_id(
        &database,
        PlayList::collection_name(),
        Service::Tidal,
        &playlist_id,
    )
    .await?
    .unwrap_or_else(generate_id);

    // The --name argument overrides the playlist's name as it is on the service.
    let name = name_override.unwrap_or_else(|| playlist_name.clone());

    let (search_terms, search_double_metaphone_codes, search_n_grams) =
        build_search_fields(&[normalise_name(&name)]);

    let playlist_out = PlayList {
        id,
        name: name.clone(),
        name_normalised: normalise_name(&name),
        name_normalised_strong: normalise_name_strong(&name),
        disambiguation: None,
        notes: description,
        data_maybe_missing: None,
        potential_duplicate: None,
        needs_review: None,
        external_service_associations: Some(vec![ExternalServiceAssociation::Tidal {
            id: playlist_id.clone(),
            image_urls: playlist_images,
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

    upsert(&database, &playlist_out).await?;

    println!(
        "Imported playlist \"{}\" with {} tracks.",
        playlist_out.name,
        playlist_out.track_ids.len()
    );

    Ok(())
}

// https://tidal.com/browse/playlist/550e8400-e29b-41d4-a716-446655440000
// https://tidal.com/playlist/550e8400-e29b-41d4-a716-446655440000
// https://listen.tidal.com/playlist/550e8400-e29b-41d4-a716-446655440000

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut values: Vec<String>) -> Vec<String> {
        values.sort();
        values
    }

    // --- parse_spotify_playlist_id ---

    #[test]
    fn parse_playlist_id_from_plain_playlist_url() {
        assert_eq!(
            parse_spotify_playlist_id("https://open.spotify.com/playlist/3y1VsqLEvW4vUQvIAVjT1P")
                .unwrap(),
            "3y1VsqLEvW4vUQvIAVjT1P"
        );
    }

    #[test]
    fn parse_playlist_id_from_user_playlist_url() {
        assert_eq!(
            parse_spotify_playlist_id(
                "https://open.spotify.com/user/1255231405/playlist/1nanrCTj3UhGJanaIVPyi8"
            )
            .unwrap(),
            "1nanrCTj3UhGJanaIVPyi8"
        );
    }

    #[test]
    fn parse_playlist_id_from_play_spotify_user_playlist_url() {
        assert_eq!(
            parse_spotify_playlist_id(
                "https://play.spotify.com/user/1277013959/playlist/0j0RUMi4syrTPG6jtHPNux"
            )
            .unwrap(),
            "0j0RUMi4syrTPG6jtHPNux"
        );
    }

    #[test]
    fn parse_playlist_id_ignores_query_string() {
        assert_eq!(
            parse_spotify_playlist_id(
                "https://open.spotify.com/playlist/3y1VsqLEvW4vUQvIAVjT1P?si=abc123"
            )
            .unwrap(),
            "3y1VsqLEvW4vUQvIAVjT1P"
        );
    }

    #[test]
    fn parse_playlist_id_rejects_trailing_path_segment() {
        // The id must be the final path segment: trailing garbage means the last two
        // segments are ("<id>", "extra"), which is not a playlist reference.
        assert!(
            parse_spotify_playlist_id(
                "https://open.spotify.com/playlist/3y1VsqLEvW4vUQvIAVjT1P/extra"
            )
            .is_err()
        );
    }

    #[test]
    fn parse_playlist_id_rejects_too_few_segments() {
        assert!(parse_spotify_playlist_id("https://open.spotify.com/playlist").is_err());
        assert!(parse_spotify_playlist_id("https://open.spotify.com").is_err());
        assert!(parse_spotify_playlist_id("https://open.spotify.com/").is_err());
    }

    #[test]
    fn parse_playlist_id_rejects_non_playlist_paths() {
        assert!(
            parse_spotify_playlist_id("https://open.spotify.com/track/3y1VsqLEvW4vUQvIAVjT1P")
                .is_err()
        );
        assert!(
            parse_spotify_playlist_id("https://open.spotify.com/album/3y1VsqLEvW4vUQvIAVjT1P")
                .is_err()
        );
    }

    #[test]
    fn parse_playlist_id_rejects_non_url_input() {
        assert!(parse_spotify_playlist_id("not a url at all").is_err());
    }

    // --- build_search_fields ---

    #[test]
    fn build_search_fields_splits_terms_and_dedups_across_strings() {
        // "world" appears in both strings and must only be indexed once.
        let (terms, metaphones, n_grams) =
            build_search_fields(&["hello world".to_string(), "world".to_string()]);

        assert_eq!(sorted(terms), vec!["hello", "world"]);

        // Double Metaphone: "hello" -> HL; "world" -> ARLT (primary) / FRLT (alternate).
        assert_eq!(sorted(metaphones), vec!["ARLT", "FRLT", "HL"]);

        // 2- and 3-grams of "hello" and "world", deduplicated.
        assert_eq!(
            sorted(n_grams),
            vec![
                "el", "ell", "he", "hel", "ld", "ll", "llo", "lo", "or", "orl", "rl", "rld", "wo",
                "wor"
            ]
        );
    }

    #[test]
    fn build_search_fields_empty_input() {
        let (terms, metaphones, n_grams) = build_search_fields(&[]);
        assert!(terms.is_empty());
        assert!(metaphones.is_empty());
        assert!(n_grams.is_empty());
    }

    // --- images_to_image_urls ---

    fn images(urls: &[&str]) -> Vec<SpImage> {
        urls.iter()
            .map(|u| SpImage { url: u.to_string() })
            .collect()
    }

    #[test]
    fn images_to_image_urls_empty_is_none() {
        assert!(images_to_image_urls(&[]).is_none());
    }

    #[test]
    fn images_to_image_urls_single_image_fills_large_and_small() {
        let result = images_to_image_urls(&images(&["a"])).unwrap();
        assert_eq!(result.large.as_deref(), Some("a"));
        assert_eq!(result.small.as_deref(), Some("a"));
        assert_eq!(result.medium, None);
    }

    #[test]
    fn images_to_image_urls_two_images_first_large_last_small() {
        let result = images_to_image_urls(&images(&["big", "tiny"])).unwrap();
        assert_eq!(result.large.as_deref(), Some("big"));
        assert_eq!(result.small.as_deref(), Some("tiny"));
        assert_eq!(result.medium, None);
    }

    #[test]
    fn images_to_image_urls_three_images_middle_is_medium() {
        let result = images_to_image_urls(&images(&["big", "mid", "tiny"])).unwrap();
        assert_eq!(result.large.as_deref(), Some("big"));
        assert_eq!(result.medium.as_deref(), Some("mid"));
        assert_eq!(result.small.as_deref(), Some("tiny"));
    }

    #[test]
    fn images_to_image_urls_four_images_uses_len_over_two_as_medium() {
        // len / 2 == 2, i.e. the third image.
        let result = images_to_image_urls(&images(&["a", "b", "c", "d"])).unwrap();
        assert_eq!(result.large.as_deref(), Some("a"));
        assert_eq!(result.medium.as_deref(), Some("c"));
        assert_eq!(result.small.as_deref(), Some("d"));
    }

    // --- parse_tidal_playlist_id ---

    #[test]
    fn parse_tidal_playlist_id_from_browse_url() {
        assert_eq!(
            parse_tidal_playlist_id(
                "https://tidal.com/browse/playlist/550e8400-e29b-41d4-a716-446655440000"
            )
            .unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn parse_tidal_playlist_id_from_plain_playlist_url() {
        assert_eq!(
            parse_tidal_playlist_id(
                "https://tidal.com/playlist/550e8400-e29b-41d4-a716-446655440000"
            )
            .unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn parse_tidal_playlist_id_from_listen_subdomain_url() {
        assert_eq!(
            parse_tidal_playlist_id(
                "https://listen.tidal.com/playlist/550e8400-e29b-41d4-a716-446655440000"
            )
            .unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn parse_tidal_playlist_id_ignores_query_string() {
        assert_eq!(
            parse_tidal_playlist_id(
                "https://tidal.com/browse/playlist/550e8400-e29b-41d4-a716-446655440000?u"
            )
            .unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn parse_tidal_playlist_id_rejects_non_playlist_paths() {
        assert!(parse_tidal_playlist_id("https://tidal.com/browse/track/251380837").is_err());
        assert!(parse_tidal_playlist_id("https://tidal.com/browse/album/251380836").is_err());
    }

    #[test]
    fn parse_tidal_playlist_id_rejects_too_few_segments() {
        assert!(parse_tidal_playlist_id("https://tidal.com/playlist").is_err());
        assert!(parse_tidal_playlist_id("https://tidal.com").is_err());
    }

    #[test]
    fn parse_tidal_playlist_id_rejects_non_url_input() {
        assert!(parse_tidal_playlist_id("not a url at all").is_err());
    }

    // --- tidal_iso8601_duration_secs ---

    #[test]
    fn iso8601_duration_parses_minutes_and_seconds() {
        assert_eq!(tidal_iso8601_duration_secs("PT3M58S"), Some(238.0));
        assert_eq!(tidal_iso8601_duration_secs("PT45S"), Some(45.0));
    }

    #[test]
    fn iso8601_duration_parses_hours_minutes_seconds() {
        assert_eq!(tidal_iso8601_duration_secs("PT1H2M3S"), Some(3723.0));
        assert_eq!(tidal_iso8601_duration_secs("PT2H"), Some(7200.0));
    }

    #[test]
    fn iso8601_duration_parses_zero_and_fractional_seconds() {
        assert_eq!(tidal_iso8601_duration_secs("PT0S"), Some(0.0));
        assert_eq!(tidal_iso8601_duration_secs("PT3.5S"), Some(3.5));
    }

    #[test]
    fn iso8601_duration_date_only_is_zero() {
        // No time component (nothing after `T`, or no `T` at all) => contributes no seconds.
        assert_eq!(tidal_iso8601_duration_secs("P1D"), Some(0.0));
    }

    #[test]
    fn iso8601_duration_rejects_malformed() {
        assert_eq!(tidal_iso8601_duration_secs(""), None);
        assert_eq!(tidal_iso8601_duration_secs("3M58S"), None); // missing leading P
        assert_eq!(tidal_iso8601_duration_secs("PT3X"), None); // unknown designator
        assert_eq!(tidal_iso8601_duration_secs("PT12"), None); // trailing number, no unit
    }

    // --- tidal_artwork_image_urls ---

    fn tidal_resource(
        kind: &str,
        id: &str,
        attributes: serde_json::Value,
        relationships: serde_json::Value,
    ) -> TidalResource {
        TidalResource {
            kind: kind.to_string(),
            id: id.to_string(),
            attributes,
            relationships,
        }
    }

    #[test]
    fn tidal_artwork_image_urls_sorts_by_width() {
        // Files are deliberately unordered; the helper must sort by width so smallest
        // maps to `small`, largest to `large`, and the middle to `medium`.
        let artwork = tidal_resource(
            "artworks",
            "a1",
            serde_json::json!({
                "files": [
                    { "href": "https://img/large.jpg",  "meta": { "width": 640, "height": 640 } },
                    { "href": "https://img/small.jpg",  "meta": { "width": 80,  "height": 80 } },
                    { "href": "https://img/medium.jpg", "meta": { "width": 320, "height": 320 } },
                ]
            }),
            serde_json::Value::Null,
        );
        let urls = tidal_artwork_image_urls(&artwork).unwrap();
        assert_eq!(urls.small.as_deref(), Some("https://img/small.jpg"));
        assert_eq!(urls.medium.as_deref(), Some("https://img/medium.jpg"));
        assert_eq!(urls.large.as_deref(), Some("https://img/large.jpg"));
    }

    #[test]
    fn tidal_artwork_image_urls_single_file_fills_large_and_small() {
        let artwork = tidal_resource(
            "artworks",
            "a1",
            serde_json::json!({ "files": [ { "href": "https://img/only.jpg", "meta": { "width": 100 } } ] }),
            serde_json::Value::Null,
        );
        let urls = tidal_artwork_image_urls(&artwork).unwrap();
        assert_eq!(urls.large.as_deref(), Some("https://img/only.jpg"));
        assert_eq!(urls.small.as_deref(), Some("https://img/only.jpg"));
        assert_eq!(urls.medium, None);
    }

    #[test]
    fn tidal_artwork_image_urls_empty_or_absent_is_none() {
        let empty = tidal_resource(
            "artworks",
            "a1",
            serde_json::json!({ "files": [] }),
            serde_json::Value::Null,
        );
        assert!(tidal_artwork_image_urls(&empty).is_none());
        let no_files = tidal_resource(
            "artworks",
            "a1",
            serde_json::json!({}),
            serde_json::Value::Null,
        );
        assert!(tidal_artwork_image_urls(&no_files).is_none());
    }

    // --- tidal_rel_ids ---

    #[test]
    fn tidal_rel_ids_handles_array_and_single_object_forms() {
        let res = tidal_resource(
            "tracks",
            "t1",
            serde_json::Value::Null,
            serde_json::json!({
                "artists": { "data": [ { "type": "artists", "id": "a1" }, { "type": "artists", "id": "a2" } ] },
                "albums": { "data": { "type": "albums", "id": "al1" } },
            }),
        );
        assert_eq!(tidal_rel_ids(&res, "artists"), vec!["a1", "a2"]);
        assert_eq!(tidal_rel_ids(&res, "albums"), vec!["al1"]);
        assert!(tidal_rel_ids(&res, "coverArt").is_empty());
    }

    // --- tidal_next_cursor ---

    #[test]
    fn tidal_next_cursor_prefers_meta_next_cursor() {
        let links = TidalLinks {
            next: Some("/playlists/x/relationships/items?page[cursor]=ignored".to_string()),
            meta: TidalLinksMeta {
                next_cursor: Some("fromMeta".to_string()),
            },
        };
        assert_eq!(tidal_next_cursor(&links).as_deref(), Some("fromMeta"));
    }

    #[test]
    fn tidal_next_cursor_parses_links_next() {
        let links = TidalLinks {
            next: Some(
                "/playlists/x/relationships/items?countryCode=AU&page[cursor]=zyx".to_string(),
            ),
            meta: TidalLinksMeta::default(),
        };
        assert_eq!(tidal_next_cursor(&links).as_deref(), Some("zyx"));

        // Cursor followed by a further query parameter must stop at the `&`.
        let links = TidalLinks {
            next: Some(
                "/playlists/x/relationships/items?page[cursor]=mid&countryCode=AU".to_string(),
            ),
            meta: TidalLinksMeta::default(),
        };
        assert_eq!(tidal_next_cursor(&links).as_deref(), Some("mid"));
    }

    #[test]
    fn tidal_next_cursor_none_when_no_next() {
        assert_eq!(tidal_next_cursor(&TidalLinks::default()), None);
    }

    #[test]
    fn tidal_next_cursor_decodes_cursor_parsed_from_links_next() {
        // A cursor carrying percent-encoded bytes in links.next is returned decoded, so
        // the caller re-encodes it exactly once when issuing the next request.
        let links = TidalLinks {
            next: Some("/x?page[cursor]=aa%2Bbb%3D".to_string()),
            meta: TidalLinksMeta::default(),
        };
        assert_eq!(tidal_next_cursor(&links).as_deref(), Some("aa+bb="));
    }

    // --- percent_decode ---

    #[test]
    fn percent_decode_handles_escapes_plus_and_plain() {
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("x+y"), "x y");
        assert_eq!(percent_decode("%2Fpath%2F"), "/path/");
        // A malformed trailing escape is left verbatim.
        assert_eq!(percent_decode("bad%zz"), "bad%zz");
        assert_eq!(percent_decode("trailing%"), "trailing%");
    }

    // --- parse_tidal_playlist_id trailing slash ---

    #[test]
    fn parse_tidal_playlist_id_accepts_trailing_slash() {
        assert_eq!(
            parse_tidal_playlist_id(
                "https://tidal.com/browse/playlist/550e8400-e29b-41d4-a716-446655440000/"
            )
            .unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    // --- tidal_owner ---

    fn tidal_playlist_doc(
        relationships: serde_json::Value,
        included: Vec<TidalResource>,
    ) -> TidalSingleDoc {
        TidalSingleDoc {
            data: tidal_resource("playlists", "p1", serde_json::Value::Null, relationships),
            included,
        }
    }

    #[test]
    fn tidal_owner_prefers_owner_profiles_and_reads_name() {
        // ownerProfiles wins over owners, and the profile's name is used.
        let doc = tidal_playlist_doc(
            serde_json::json!({
                "ownerProfiles": { "data": [ { "type": "artists", "id": "op1" } ] },
                "owners": { "data": [ { "type": "users", "id": "u1" } ] },
            }),
            vec![tidal_resource(
                "artists",
                "op1",
                serde_json::json!({ "name": "DJ Owner" }),
                serde_json::Value::Null,
            )],
        );
        assert_eq!(
            tidal_owner(&doc),
            Some(("op1".to_string(), Some("DJ Owner".to_string())))
        );
    }

    #[test]
    fn tidal_owner_empty_name_yields_no_name() {
        // An empty profile name (common under client credentials) -> None, so the caller
        // falls back to the owner id.
        let doc = tidal_playlist_doc(
            serde_json::json!({ "ownerProfiles": { "data": [ { "type": "artists", "id": "op1" } ] } }),
            vec![tidal_resource(
                "artists",
                "op1",
                serde_json::json!({ "name": "" }),
                serde_json::Value::Null,
            )],
        );
        assert_eq!(tidal_owner(&doc), Some(("op1".to_string(), None)));
    }

    #[test]
    fn tidal_owner_falls_back_to_owners_and_then_none() {
        // No ownerProfiles -> fall back to owners (no embedded resource -> no name).
        let doc = tidal_playlist_doc(
            serde_json::json!({ "owners": { "data": [ { "type": "users", "id": "u1" } ] } }),
            vec![],
        );
        assert_eq!(tidal_owner(&doc), Some(("u1".to_string(), None)));

        // Neither relationship populated -> no owner at all.
        let doc = tidal_playlist_doc(serde_json::json!({ "owners": { "data": [] } }), vec![]);
        assert_eq!(tidal_owner(&doc), None);
    }
}
