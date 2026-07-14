//! Integration tests for the playlist CLI commands.
//!
//! These tests require a MongoDB instance at mongodb://localhost:27017 and MUST be run
//! single-threaded because they mutate process environment variables:
//!
//! ```sh
//! cargo test -p playlist-cli --test cli_integration -- --include-ignored --test-threads=1
//! ```
//!
//! Each test uses its own database (a `playlist_test_cli*` name) and drops it first, so
//! runs are idempotent.

use futures::TryStreamExt;
use mongodb::Database;
use mongodb::bson::doc;
use playlist_cli::commands::{
    import_playlist::import_playlist, migrate, set_compiler_name::set_compiler_name,
};
use playlist_core::models::{
    ExternalServiceAssociation, MusicItemBase,
    album::Album,
    artist::Artist,
    compiler::Compiler,
    playlist::PlayList,
    track::{LinkedTrack, Track},
};
use serde_json::json;
use wiremock::matchers::{basic_auth, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MONGO_URI: &str = "mongodb://localhost:27017";

/// Set an environment variable.
///
/// SAFETY: `std::env::set_var` is unsafe in edition 2024 because it is not thread-safe.
/// This suite is documented to run with `--test-threads=1` (see the module docs and each
/// test's `#[ignore]` message), so no other thread reads or writes the environment
/// concurrently.
fn set_env(key: &str, value: &str) {
    unsafe { std::env::set_var(key, value) }
}

/// Remove an environment variable. SAFETY: as for [`set_env`].
fn remove_env(key: &str) {
    unsafe { std::env::remove_var(key) }
}

/// WARNING: any test that (transitively) calls `playlist_core::get_database()` MUST run
/// its body on this shared runtime via [`run_on_shared_runtime`].
///
/// `get_database` caches a process-global `mongodb::Client` whose background monitor
/// tasks are spawned on the tokio runtime that first initialises it. `#[tokio::test]`
/// gives every test its own runtime, so once the initialising test's runtime is dropped
/// the cached client becomes unusable and every later `get_database`-based test fails
/// with a server-selection timeout. Running all such tests on this single, never-dropped
/// runtime keeps the cached client's monitor tasks alive for the whole test binary.
/// (Same pattern as crates/core/tests/db_integration.rs.)
///
/// Currently the `get_database`-dependent tests are the migrate ones: `migrate::run` and
/// `migrate::build_linked_tracks` load tracks through
/// `playlist_core::models::server::load_music_items`, which uses `get_database`.
fn shared_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("failed to create runtime"))
}

/// Runs the test body on the shared runtime (see [`shared_runtime`]), propagating panics.
async fn run_on_shared_runtime<F>(test_body: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    if let Err(err) = shared_runtime().spawn(test_body).await {
        match err.try_into_panic() {
            Ok(panic) => std::panic::resume_unwind(panic),
            Err(err) => panic!("test task failed: {err}"),
        }
    }
}

/// Connect to the local MongoDB and return a freshly dropped database with this name.
async fn fresh_db(name: &str) -> Database {
    let client = mongodb::Client::with_uri_str(MONGO_URI)
        .await
        .expect("failed to connect to MongoDB at localhost:27017");
    let db = client.database(name);
    db.drop().await.expect("failed to drop test database");
    db
}

async fn collect<T>(db: &Database, collection: &str) -> Vec<T>
where
    T: serde::de::DeserializeOwned + Send + Sync,
{
    db.collection::<T>(collection)
        .find(doc! {})
        .await
        .expect("find failed")
        .try_collect()
        .await
        .expect("cursor failed")
}

fn is_generated_id(id: &str) -> bool {
    id.len() == 17 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Return the Spotify id from an item's external service associations, if any.
fn spotify_id<T: MusicItemBase>(item: &T) -> Option<String> {
    item.external_service_associations()?
        .iter()
        .find_map(|assoc| match assoc {
            ExternalServiceAssociation::Spotify { id, .. } => Some(id.clone()),
            _ => None,
        })
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

// ---------------------------------------------------------------------------
// Spotify mock fixture
// ---------------------------------------------------------------------------

const PLAYLIST_ID: &str = "pl123abc";
const OWNER_ID: &str = "user_owner_1";
const CLIENT_ID: &str = "test-client-id";
const CLIENT_SECRET: &str = "test-client-secret";
const ACCESS_TOKEN: &str = "test-access-token";

/// Stand up a wiremock server that mimics the Spotify endpoints the importer hits, and
/// point the importer's env-var overrides at it.
///
/// All data endpoints require the Bearer token issued by the mock token endpoint, and
/// the playlist/tracks endpoints require `market=AU`, so the OAuth round-trip and market
/// propagation are verified on every import.
///
/// The fixture playlist has four items spread over two pages (to exercise pagination):
/// - page 1 (next = non-null): one importable track ("Poker Face", 238s, two artists,
///   one album) and one item with `"track": null` (e.g. removed episode) -> skipped;
/// - page 2 (next = null, must be fetched at least once): one local file (track with
///   `"id": null`) -> skipped, and a second importable track ("Bad Romance", 295s,
///   reusing one of page 1's artists and its album to exercise cross-page dedup).
async fn mock_spotify() -> MockServer {
    mock_spotify_with_description("Party playlist!").await
}

/// As [`mock_spotify`], with the playlist `description` field set to the given value.
async fn mock_spotify_with_description(description: &str) -> MockServer {
    let server = MockServer::start().await;

    // Token endpoint (client-credentials flow): the client credentials must arrive as
    // HTTP basic auth.
    Mock::given(method("POST"))
        .and(path("/api/token"))
        .and(basic_auth(CLIENT_ID, CLIENT_SECRET))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": ACCESS_TOKEN,
            "token_type": "Bearer",
            "expires_in": 3600,
        })))
        .mount(&server)
        .await;

    // Playlist metadata.
    Mock::given(method("GET"))
        .and(path(format!("/playlists/{PLAYLIST_ID}")))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN}")))
        .and(query_param("market", "AU"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": PLAYLIST_ID,
            "name": "Just Dance 2020",
            "description": description,
            "images": [
                { "url": "https://img.example/pl-large.jpg" },
                { "url": "https://img.example/pl-small.jpg" },
            ],
            "owner": { "id": OWNER_ID, "display_name": "Owner Fallback" },
        })))
        .mount(&server)
        .await;

    // Tracks page 1: two items, next page present. "next" points back at this mock
    // server so that an importer that followed the URL literally could never silently
    // hit the real Spotify API from the test suite.
    Mock::given(method("GET"))
        .and(path(format!("/playlists/{PLAYLIST_ID}/tracks")))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN}")))
        .and(query_param("market", "AU"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {
                    "track": {
                        "id": "sp_track_1",
                        "name": "Poker Face",
                        "duration_ms": 238000,
                        "artists": [
                            { "id": "sp_artist_1", "name": "Lady Gaga" },
                            { "id": "sp_artist_2", "name": "RedOne" },
                        ],
                        "album": {
                            "id": "sp_album_1",
                            "name": "The Fame",
                            "images": [ { "url": "https://img.example/album.jpg" } ],
                            "artists": [ { "id": "sp_artist_1", "name": "Lady Gaga" } ],
                        },
                    },
                },
                // Unavailable item: must be skipped.
                { "track": null },
            ],
            "next": format!(
                "{}/playlists/{PLAYLIST_ID}/tracks?market=AU&limit=100&offset=2",
                server.uri()
            ),
        })))
        .mount(&server)
        .await;

    // Tracks page 2: a local file (skipped) followed by a second importable track, last
    // page. `.expect(1..)` fails the test if pagination regresses and this page is never
    // fetched (1.. rather than 1 because the idempotency test imports twice).
    Mock::given(method("GET"))
        .and(path(format!("/playlists/{PLAYLIST_ID}/tracks")))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN}")))
        .and(query_param("market", "AU"))
        .and(query_param("offset", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {
                    "track": {
                        "id": null,
                        "name": "Local Song",
                        "duration_ms": 60000,
                        "artists": [],
                        "album": null,
                    },
                },
                {
                    "track": {
                        "id": "sp_track_2",
                        "name": "Bad Romance",
                        "duration_ms": 295000,
                        "artists": [ { "id": "sp_artist_1", "name": "Lady Gaga" } ],
                        "album": {
                            "id": "sp_album_1",
                            "name": "The Fame",
                            "images": [ { "url": "https://img.example/album.jpg" } ],
                            "artists": [ { "id": "sp_artist_1", "name": "Lady Gaga" } ],
                        },
                    },
                },
            ],
            "next": null,
        })))
        .expect(1..)
        .mount(&server)
        .await;

    // Owner's user profile.
    Mock::given(method("GET"))
        .and(path(format!("/users/{OWNER_ID}")))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "display_name": "JD Curator",
            "images": [
                { "url": "https://img.example/owner-1.jpg" },
                { "url": "https://img.example/owner-2.jpg" },
            ],
        })))
        .mount(&server)
        .await;

    server
}

/// Point the importer at the mock server via its documented env-var overrides.
fn configure_spotify_env(server: &MockServer) {
    set_env("SPOTIFY_API_BASE", &server.uri());
    set_env("SPOTIFY_TOKEN_URL", &format!("{}/api/token", server.uri()));
    set_env("SPOTIFY_CLIENT_ID", CLIENT_ID);
    set_env("SPOTIFY_CLIENT_SECRET", CLIENT_SECRET);
    set_env("SPOTIFY_MARKET", "AU");
    // Make sure ambient configuration doesn't leak into the user_id fallback logic.
    remove_env("IMPORT_USER_ID");
}

fn playlist_url() -> String {
    format!("https://open.spotify.com/playlist/{PLAYLIST_ID}")
}

// ---------------------------------------------------------------------------
// import_playlist
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn import_playlist_imports_playlist_tracks_artists_album_and_compiler() {
    let db = fresh_db("playlist_test_cli_import").await;
    let server = mock_spotify().await;
    configure_spotify_env(&server);

    import_playlist(db.clone(), &playlist_url(), None, None, Some(1600000000.0))
        .await
        .expect("import failed");

    // --- Playlist ---
    let playlists: Vec<PlayList> = collect(&db, "playlist").await;
    assert_eq!(playlists.len(), 1);
    let playlist = &playlists[0];
    assert!(
        is_generated_id(&playlist.id),
        "playlist id: {}",
        playlist.id
    );
    assert_eq!(playlist.name, "Just Dance 2020");
    assert_eq!(playlist.name_normalised, "just dance 2020");
    assert_eq!(playlist.name_normalised_strong, "just dance 2020");
    // Non-empty description -> stored as notes.
    assert_eq!(playlist.notes.as_deref(), Some("Party playlist!"));
    assert_eq!(playlist.date, Some(1600000000.0));
    // The two unimportable items (track: null, id: null) are skipped; the importable
    // track on each of the two pages remains (a missing page 2 would fail here).
    assert_eq!(playlist.track_ids.len(), 2);
    // Duration sums only the imported tracks, across both pages.
    assert_eq!(playlist.duration, 238.0 + 295.0);
    // No user_id given anywhere -> falls back to the Spotify owner id.
    assert_eq!(playlist.user_id, OWNER_ID);
    assert_eq!(spotify_id(playlist).as_deref(), Some(PLAYLIST_ID));

    // Playlist images: two images -> first = large, last = small, no medium.
    let Some([ExternalServiceAssociation::Spotify { image_urls, .. }]) =
        playlist.external_service_associations.as_deref()
    else {
        panic!("expected exactly one Spotify association on the playlist");
    };
    let image_urls = image_urls.as_ref().expect("playlist should have images");
    assert_eq!(
        image_urls.large.as_deref(),
        Some("https://img.example/pl-large.jpg")
    );
    assert_eq!(
        image_urls.small.as_deref(),
        Some("https://img.example/pl-small.jpg")
    );
    assert_eq!(image_urls.medium, None);

    // Playlist search fields are derived from the normalised name.
    assert_eq!(
        sorted(playlist.search_terms.clone()),
        vec!["2020", "dance", "just"]
    );
    assert_eq!(
        sorted(playlist.search_double_metaphone_codes.clone()),
        vec!["AST", "JST", "TNS"]
    );
    assert_eq!(
        sorted(playlist.search_n_grams.clone()),
        vec![
            "02", "020", "20", "202", "an", "anc", "ce", "da", "dan", "ju", "jus", "nc", "nce",
            "st", "us", "ust"
        ]
    );

    // --- Tracks (one importable track per page) ---
    let tracks: Vec<Track> = collect(&db, "track").await;
    assert_eq!(tracks.len(), 2);
    let track = tracks
        .iter()
        .find(|t| spotify_id(*t).as_deref() == Some("sp_track_1"))
        .expect("page-1 track doc");
    let track2 = tracks
        .iter()
        .find(|t| spotify_id(*t).as_deref() == Some("sp_track_2"))
        .expect("page-2 track doc");
    // The playlist references both tracks in Spotify (page) order.
    assert_eq!(
        playlist.track_ids,
        vec![track.id.clone(), track2.id.clone()]
    );
    assert!(is_generated_id(&track.id), "track id: {}", track.id);
    assert_eq!(track.name, "Poker Face");
    assert_eq!(track.name_normalised, "poker face");
    assert_eq!(track.duration, Some(238.0));
    assert_eq!(sorted(track.search_terms.clone()), vec!["face", "poker"]);
    assert!(!track.search_double_metaphone_codes.is_empty());
    assert!(!track.search_n_grams.is_empty());
    assert!(is_generated_id(&track2.id), "track id: {}", track2.id);
    assert_eq!(track2.name, "Bad Romance");
    assert_eq!(track2.name_normalised, "bad romance");
    assert_eq!(track2.duration, Some(295.0));
    assert_eq!(sorted(track2.search_terms.clone()), vec!["bad", "romance"]);

    // --- Artists (deduplicated across both tracks, their albums, and both pages) ---
    let artists: Vec<Artist> = collect(&db, "artist").await;
    assert_eq!(artists.len(), 2);
    let gaga = artists
        .iter()
        .find(|a| spotify_id(*a).as_deref() == Some("sp_artist_1"))
        .expect("Lady Gaga artist doc");
    let redone = artists
        .iter()
        .find(|a| spotify_id(*a).as_deref() == Some("sp_artist_2"))
        .expect("RedOne artist doc");
    assert_eq!(gaga.name, "Lady Gaga");
    assert_eq!(gaga.name_normalised, "lady gaga");
    assert_eq!(redone.name, "RedOne");
    assert!(is_generated_id(&gaga.id) && is_generated_id(&redone.id));
    // Each track references the artists' generated ids, in Spotify order.
    assert_eq!(track.artist_ids, vec![gaga.id.clone(), redone.id.clone()]);
    assert_eq!(track2.artist_ids, vec![gaga.id.clone()]);

    // --- Album (shared by both tracks, deduplicated across pages) ---
    let albums: Vec<Album> = collect(&db, "album").await;
    assert_eq!(albums.len(), 1);
    let album = &albums[0];
    assert!(is_generated_id(&album.id));
    assert_eq!(album.name, "The Fame");
    assert_eq!(spotify_id(album).as_deref(), Some("sp_album_1"));
    assert_eq!(track.album_id, Some(album.id.clone()));
    assert_eq!(track2.album_id, Some(album.id.clone()));
    assert_eq!(album.artist_ids, vec![gaga.id.clone()]);
    // One album image -> large == small, no medium.
    let Some([ExternalServiceAssociation::Spotify { image_urls, .. }]) =
        album.external_service_associations.as_deref()
    else {
        panic!("expected exactly one Spotify association on the album");
    };
    let image_urls = image_urls.as_ref().expect("album should have images");
    assert_eq!(
        image_urls.large.as_deref(),
        Some("https://img.example/album.jpg")
    );
    assert_eq!(
        image_urls.small.as_deref(),
        Some("https://img.example/album.jpg")
    );
    assert_eq!(image_urls.medium, None);

    // --- Compiler (playlist owner) ---
    let compilers: Vec<Compiler> = collect(&db, "compiler").await;
    assert_eq!(compilers.len(), 1);
    let compiler = &compilers[0];
    assert!(is_generated_id(&compiler.id));
    assert_eq!(spotify_id(compiler).as_deref(), Some(OWNER_ID));
    // The user-profile display name wins over the owner display name.
    assert_eq!(compiler.name, "JD Curator");
    assert_eq!(playlist.compiler_ids, vec![compiler.id.clone()]);
    let Some([ExternalServiceAssociation::Spotify { image_urls, .. }]) =
        compiler.external_service_associations.as_deref()
    else {
        panic!("expected exactly one Spotify association on the compiler");
    };
    let image_urls = image_urls
        .as_ref()
        .expect("compiler should have profile images");
    assert_eq!(
        image_urls.large.as_deref(),
        Some("https://img.example/owner-1.jpg")
    );
    assert_eq!(
        image_urls.small.as_deref(),
        Some("https://img.example/owner-2.jpg")
    );
    assert_eq!(image_urls.medium, None);
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn import_playlist_twice_is_idempotent() {
    let db = fresh_db("playlist_test_cli_import_idem").await;
    let server = mock_spotify().await;
    configure_spotify_env(&server);

    import_playlist(db.clone(), &playlist_url(), None, None, None)
        .await
        .expect("first import failed");

    let playlists: Vec<PlayList> = collect(&db, "playlist").await;
    let tracks: Vec<Track> = collect(&db, "track").await;
    assert_eq!(playlists.len(), 1);
    assert_eq!(tracks.len(), 2);
    let first_playlist_id = playlists[0].id.clone();
    let first_track_ids = sorted(tracks.iter().map(|t| t.id.clone()).collect());

    import_playlist(db.clone(), &playlist_url(), None, None, None)
        .await
        .expect("second import failed");

    // Counts are unchanged: everything was matched by its Spotify id and replaced.
    let playlists: Vec<PlayList> = collect(&db, "playlist").await;
    let tracks: Vec<Track> = collect(&db, "track").await;
    let artists: Vec<Artist> = collect(&db, "artist").await;
    let albums: Vec<Album> = collect(&db, "album").await;
    let compilers: Vec<Compiler> = collect(&db, "compiler").await;
    assert_eq!(playlists.len(), 1);
    assert_eq!(tracks.len(), 2);
    assert_eq!(artists.len(), 2);
    assert_eq!(albums.len(), 1);
    assert_eq!(compilers.len(), 1);

    // The find_existing_id path keeps the same generated _ids across imports.
    assert_eq!(playlists[0].id, first_playlist_id);
    assert_eq!(
        sorted(tracks.iter().map(|t| t.id.clone()).collect()),
        first_track_ids
    );
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn import_playlist_applies_name_and_user_id_overrides() {
    let db = fresh_db("playlist_test_cli_import_override").await;
    // This scenario's playlist has an empty description, which must not become notes.
    let server = mock_spotify_with_description("").await;
    configure_spotify_env(&server);
    // The explicit user_id argument outranks IMPORT_USER_ID (top rung of the
    // arg > env > Spotify-owner precedence in do_spotify_import).
    set_env("IMPORT_USER_ID", "envUser42");

    let result = import_playlist(
        db.clone(),
        &playlist_url(),
        Some("customUser123".to_string()),
        Some("My Renamed List".to_string()),
        None,
    )
    .await;
    // Clean up before asserting so a failure cannot leak the var into later tests.
    remove_env("IMPORT_USER_ID");
    result.expect("import failed");

    let playlists: Vec<PlayList> = collect(&db, "playlist").await;
    assert_eq!(playlists.len(), 1);
    let playlist = &playlists[0];
    assert_eq!(playlist.name, "My Renamed List");
    // The normalised names and search fields are recomputed from the override.
    assert_eq!(playlist.name_normalised, "my renamed list");
    assert_eq!(playlist.name_normalised_strong, "my renamed list");
    assert_eq!(
        sorted(playlist.search_terms.clone()),
        vec!["list", "my", "renamed"]
    );
    assert_eq!(playlist.user_id, "customUser123");
    assert_eq!(playlist.date, None);
    // Empty description -> filtered out of notes.
    assert_eq!(playlist.notes, None);
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn import_playlist_user_id_falls_back_to_import_user_id_env_var() {
    let db = fresh_db("playlist_test_cli_import_user_env").await;
    let server = mock_spotify().await;
    configure_spotify_env(&server);
    // Middle rung of the arg > env > Spotify-owner precedence: no user_id argument, so
    // the IMPORT_USER_ID env var must win over the Spotify owner id.
    set_env("IMPORT_USER_ID", "envUser42");

    let result = import_playlist(db.clone(), &playlist_url(), None, None, None).await;
    // Clean up before asserting so a failure cannot leak the var into later tests.
    remove_env("IMPORT_USER_ID");
    result.expect("import failed");

    let playlists: Vec<PlayList> = collect(&db, "playlist").await;
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].user_id, "envUser42");
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn import_playlist_rejects_unsupported_service_uri() {
    let db = fresh_db("playlist_test_cli_import_unsupported").await;

    let result = import_playlist(
        db.clone(),
        "https://music.example.com/playlist/abc",
        None,
        None,
        None,
    )
    .await;

    let error = result.expect_err("non-Spotify URIs must be rejected");
    assert_eq!(error.to_string(), "Unsupported service URI.");
    // Nothing was written.
    let playlists: Vec<PlayList> = collect(&db, "playlist").await;
    assert!(playlists.is_empty());
}

// ---------------------------------------------------------------------------
// migrate
// ---------------------------------------------------------------------------

/// The playlist group id migrate uses to select which playlists to migrate.
const JD_GROUP_ID: &str = "zmWKoBuAoSLCWDvzn";

/// `migrate::run` reads tracks back through `playlist_core::get_database()` when it
/// builds linked tracks, so the app env vars must point at the test database too.
fn configure_db_env(db_name: &str) {
    set_env("DB_CONNECTION_STRING", MONGO_URI);
    set_env("DB_NAME", db_name);
}

// Both migrate tests (transitively) call `playlist_core::get_database()`, so their
// bodies MUST run on the shared runtime — see [`shared_runtime`] for why.

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn migrate_converts_old_camel_case_collections() {
    run_on_shared_runtime(migrate_converts_old_camel_case_collections_scenario()).await;
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn build_linked_tracks_groups_by_strong_name_and_shared_artist() {
    run_on_shared_runtime(build_linked_tracks_groups_by_strong_name_and_shared_artist_scenario())
        .await;
}

async fn migrate_converts_old_camel_case_collections_scenario() {
    let db_name = "playlist_test_cli_migrate";
    let db = fresh_db(db_name).await;
    configure_db_env(db_name);

    // Seed old-format (camelCase) collections.
    db.collection("Compiler")
        .insert_many(vec![doc! {
            "_id": "comp1",
            "name": "DJ Cool",
            "spotifyId": "sp_comp_1",
            "imageURLs": { "small": "https://img/c-small.jpg", "large": "https://img/c-large.jpg" },
            "appearsInPlayLists": ["pl1"],
        }])
        .await
        .unwrap();
    db.collection("PlayList")
        .insert_many(vec![
            doc! {
                "_id": "pl1",
                "name": "Dance Hits",
                "groupId": JD_GROUP_ID,
                "compilerIds": ["comp1"],
                "trackIds": ["tr1", "tr2"],
                "duration": 300.0,
                "userId": "user1",
                "date": 1500000000.0,
            },
            // Not in the JD group: must NOT be migrated.
            doc! {
                "_id": "pl2",
                "name": "Other Group List",
                "groupId": "someOtherGroup",
                "trackIds": ["tr3"],
                "userId": "user2",
            },
        ])
        .await
        .unwrap();
    db.collection("Track")
        .insert_many(vec![
            doc! {
                "_id": "tr1",
                "name": "Song One",
                "artistIds": ["ar1"],
                "albumId": "al1",
                "duration": 180.0,
                "spotifyId": "sp_tr_1",
            },
            doc! {
                "_id": "tr2",
                "name": "Song Two (Remix)",
                "artistIds": ["ar2"],
                "albumId": "al1",
                "duration": 120.0,
            },
            // Only referenced by the non-migrated playlist: must NOT be migrated.
            doc! {
                "_id": "tr3",
                "name": "Unreferenced Song",
                "artistIds": ["ar1"],
            },
        ])
        .await
        .unwrap();
    db.collection("Artist")
        .insert_many(vec![
            doc! { "_id": "ar1", "name": "Artist Alpha", "spotifyId": "sp_ar_1" },
            doc! { "_id": "ar2", "name": "Artist Beta", "mbId": "mb_ar_2" },
            // Not referenced by any migrated track: must NOT be migrated.
            doc! { "_id": "ar3", "name": "Unused Artist" },
        ])
        .await
        .unwrap();
    db.collection("Album")
        .insert_many(vec![
            doc! { "_id": "al1", "name": "Album One", "spotifyId": "sp_al_1", "artistIds": ["ar1"] },
            // Not referenced by any migrated track: must NOT be migrated.
            doc! { "_id": "al2", "name": "Unused Album" },
        ])
        .await
        .unwrap();

    migrate::run(db.clone()).await.expect("migration failed");

    // --- Compiler ---
    let compilers: Vec<Compiler> = collect(&db, "compiler").await;
    assert_eq!(compilers.len(), 1);
    let compiler = &compilers[0];
    assert_eq!(compiler.id, "comp1");
    assert_eq!(compiler.name, "DJ Cool");
    assert_eq!(compiler.name_normalised, "dj cool");
    assert_eq!(spotify_id(compiler).as_deref(), Some("sp_comp_1"));
    let Some([ExternalServiceAssociation::Spotify { image_urls, .. }]) =
        compiler.external_service_associations.as_deref()
    else {
        panic!("expected exactly one Spotify association on the compiler");
    };
    let image_urls = image_urls.as_ref().expect("imageURLs should migrate");
    assert_eq!(image_urls.small.as_deref(), Some("https://img/c-small.jpg"));
    assert_eq!(image_urls.large.as_deref(), Some("https://img/c-large.jpg"));
    assert_eq!(image_urls.medium, None);
    assert_eq!(sorted(compiler.search_terms.clone()), vec!["cool", "dj"]);

    // --- Playlist (only the JD-group playlist migrates) ---
    let playlists: Vec<PlayList> = collect(&db, "playlist").await;
    assert_eq!(playlists.len(), 1);
    let playlist = &playlists[0];
    assert_eq!(playlist.id, "pl1");
    assert_eq!(playlist.name, "Dance Hits");
    assert_eq!(playlist.name_normalised, "dance hits");
    assert_eq!(playlist.compiler_ids, vec!["comp1"]);
    assert_eq!(playlist.track_ids, vec!["tr1", "tr2"]);
    assert_eq!(playlist.duration, 300.0);
    assert_eq!(playlist.user_id, "user1");
    assert_eq!(playlist.group_id.as_deref(), Some(JD_GROUP_ID));
    assert_eq!(playlist.date, Some(1500000000.0));
    // Playlist search terms include the compiler's name terms.
    assert_eq!(
        sorted(playlist.search_terms.clone()),
        vec!["cool", "dance", "dj", "hits"]
    );
    assert!(!playlist.search_double_metaphone_codes.is_empty());
    assert!(!playlist.search_n_grams.is_empty());

    // --- Tracks (only tracks referenced by migrated playlists migrate) ---
    let tracks: Vec<Track> = collect(&db, "track").await;
    assert_eq!(tracks.len(), 2);
    let track1 = tracks.iter().find(|t| t.id == "tr1").expect("tr1 migrated");
    let track2 = tracks.iter().find(|t| t.id == "tr2").expect("tr2 migrated");

    assert_eq!(track1.name, "Song One");
    assert_eq!(track1.name_normalised, "song one");
    assert_eq!(track1.artist_ids, vec!["ar1"]);
    assert_eq!(track1.album_id.as_deref(), Some("al1"));
    assert_eq!(track1.duration, Some(180.0));
    assert_eq!(spotify_id(track1).as_deref(), Some("sp_tr_1"));
    // Track search terms include the artist's name terms.
    assert_eq!(
        sorted(track1.search_terms.clone()),
        vec!["alpha", "artist", "one", "song"]
    );

    // Normalised names are recomputed with the current implementations.
    assert_eq!(track2.name, "Song Two (Remix)");
    assert_eq!(track2.name_normalised, "song two remix");
    assert_eq!(track2.name_normalised_strong, "song two");
    assert_eq!(track2.artist_ids, vec!["ar2"]);
    assert!(track2.external_service_associations.is_none());
    assert_eq!(
        sorted(track2.search_terms.clone()),
        vec!["artist", "beta", "remix", "song", "two"]
    );

    // --- Artists (only artists referenced by migrated tracks migrate) ---
    let artists: Vec<Artist> = collect(&db, "artist").await;
    assert_eq!(artists.len(), 2);
    let artist1 = artists
        .iter()
        .find(|a| a.id == "ar1")
        .expect("ar1 migrated");
    let artist2 = artists
        .iter()
        .find(|a| a.id == "ar2")
        .expect("ar2 migrated");
    assert_eq!(artist1.name_normalised, "artist alpha");
    assert_eq!(spotify_id(artist1).as_deref(), Some("sp_ar_1"));
    // mbId becomes a MusicBrainz association.
    match artist2.external_service_associations.as_deref() {
        Some([ExternalServiceAssociation::MusicBrainz { id }]) => assert_eq!(id, "mb_ar_2"),
        other => panic!("expected a single MusicBrainz association, got {:?}", other),
    }
    assert_eq!(sorted(artist2.search_terms.clone()), vec!["artist", "beta"]);

    // --- Albums (only albums referenced by migrated tracks migrate) ---
    let albums: Vec<Album> = collect(&db, "album").await;
    assert_eq!(albums.len(), 1);
    let album = &albums[0];
    assert_eq!(album.id, "al1");
    assert_eq!(album.name_normalised, "album one");
    assert_eq!(album.artist_ids, vec!["ar1"]);
    assert_eq!(spotify_id(album).as_deref(), Some("sp_al_1"));
    assert_eq!(sorted(album.search_terms.clone()), vec!["album", "one"]);

    // The two migrated tracks have different names, so no linked tracks are created.
    let linked: Vec<LinkedTrack> = collect(&db, "linked_track").await;
    assert!(linked.is_empty());
}

fn make_track(id: &str, name: &str, artist_ids: &[&str]) -> Track {
    Track {
        id: id.to_string(),
        name: name.to_string(),
        name_normalised: playlist_core::normalise_name(name),
        name_normalised_strong: playlist_core::normalise_name_strong(name),
        disambiguation: None,
        notes: None,
        data_maybe_missing: None,
        potential_duplicate: None,
        needs_review: None,
        external_service_associations: None,
        search_terms: vec![],
        search_double_metaphone_codes: vec![],
        search_n_grams: vec![],
        artist_ids: artist_ids.iter().map(|s| s.to_string()).collect(),
        album_id: None,
        duration: None,
    }
}

async fn build_linked_tracks_groups_by_strong_name_and_shared_artist_scenario() {
    let db_name = "playlist_test_cli_linked";
    let db = fresh_db(db_name).await;
    configure_db_env(db_name);

    // t1 and t2 share the strong-normalised name AND artist a1 -> linked together.
    // t3 shares the name but has no artist in common -> NOT linked.
    // t4 has a unique name (group of one) -> no linked_track doc.
    db.collection::<Track>("track")
        .insert_many(vec![
            make_track("t1", "Umbrella", &["a1"]),
            make_track("t2", "Umbrella - Radio Edit", &["a1", "a2"]),
            make_track("t3", "Umbrella", &["a9"]),
            make_track("t4", "Solo Song", &["a5"]),
        ])
        .await
        .unwrap();
    // All three "Umbrella" variants collapse to the same strong-normalised name.
    assert_eq!(
        playlist_core::normalise_name_strong("Umbrella - Radio Edit"),
        "umbrella"
    );

    // Pre-existing linked_track docs are cleared by build_linked_tracks.
    db.collection::<LinkedTrack>("linked_track")
        .insert_one(LinkedTrack {
            id: "stale0000000stale".to_string(),
            track_name_normalised_strong: "stale".to_string(),
            track_ids: vec!["x".to_string()],
            artist_ids: vec!["y".to_string()],
        })
        .await
        .unwrap();

    migrate::build_linked_tracks(db.clone())
        .await
        .expect("build_linked_tracks failed");

    let linked: Vec<LinkedTrack> = collect(&db, "linked_track").await;
    assert_eq!(linked.len(), 1, "expected exactly one linked_track doc");
    let linked_track = &linked[0];
    assert!(is_generated_id(&linked_track.id), "id: {}", linked_track.id);
    assert_eq!(linked_track.track_name_normalised_strong, "umbrella");
    assert_eq!(sorted(linked_track.track_ids.clone()), vec!["t1", "t2"]);
    // Artist ids are merged across the linked tracks.
    assert_eq!(sorted(linked_track.artist_ids.clone()), vec!["a1", "a2"]);
}

// ---------------------------------------------------------------------------
// set_compiler_name
// ---------------------------------------------------------------------------

/// A compiler doc with deliberately stale search fields, so tests can verify that
/// `set_compiler_name` rebuilds them rather than leaving them untouched.
fn make_compiler(id: &str, name: &str) -> Compiler {
    Compiler {
        id: id.to_string(),
        name: name.to_string(),
        name_normalised: playlist_core::normalise_name(name),
        name_normalised_strong: playlist_core::normalise_name_strong(name),
        disambiguation: None,
        notes: None,
        data_maybe_missing: None,
        potential_duplicate: None,
        needs_review: None,
        external_service_associations: Some(vec![ExternalServiceAssociation::Spotify {
            id: "sp_comp_9".to_string(),
            image_urls: None,
        }]),
        search_terms: vec!["stale".to_string()],
        search_double_metaphone_codes: vec!["STL".to_string()],
        search_n_grams: vec!["st".to_string(), "sta".to_string()],
    }
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn set_compiler_name_updates_name_and_rebuilds_search_fields() {
    let db = fresh_db("playlist_test_cli_set_compiler").await;
    db.collection::<Compiler>("compiler")
        .insert_one(make_compiler("comp9", "Old Name"))
        .await
        .unwrap();

    set_compiler_name(db.clone(), "comp9", "Hello World")
        .await
        .expect("set_compiler_name failed");

    let compilers: Vec<Compiler> = collect(&db, "compiler").await;
    assert_eq!(compilers.len(), 1);
    let compiler = &compilers[0];
    assert_eq!(compiler.id, "comp9");
    assert_eq!(compiler.name, "Hello World");
    assert_eq!(compiler.name_normalised, "hello world");
    assert_eq!(compiler.name_normalised_strong, "hello world");
    // The search fields are rebuilt from the new name (the stale seeded values go away).
    assert_eq!(
        sorted(compiler.search_terms.clone()),
        vec!["hello", "world"]
    );
    // Double Metaphone: "hello" -> HL; "world" -> ARLT (primary) / FRLT (alternate).
    assert_eq!(
        sorted(compiler.search_double_metaphone_codes.clone()),
        vec!["ARLT", "FRLT", "HL"]
    );
    // 2- and 3-grams of "hello" and "world".
    assert_eq!(
        sorted(compiler.search_n_grams.clone()),
        vec![
            "el", "ell", "he", "hel", "ld", "ll", "llo", "lo", "or", "orl", "rl", "rld", "wo",
            "wor"
        ]
    );
    // Fields unrelated to the name are preserved.
    assert_eq!(spotify_id(compiler).as_deref(), Some("sp_comp_9"));
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn set_compiler_name_errors_for_unknown_compiler_id() {
    let db = fresh_db("playlist_test_cli_set_compiler_missing").await;
    db.collection::<Compiler>("compiler")
        .insert_one(make_compiler("comp9", "Old Name"))
        .await
        .unwrap();

    let error = set_compiler_name(db.clone(), "no_such_id", "New Name")
        .await
        .expect_err("renaming an unknown compiler id must fail");
    assert_eq!(
        error.to_string(),
        "No compiler found with id \"no_such_id\""
    );

    // The existing compiler is untouched.
    let compilers: Vec<Compiler> = collect(&db, "compiler").await;
    assert_eq!(compilers.len(), 1);
    assert_eq!(compilers[0].name, "Old Name");
}

// ---------------------------------------------------------------------------
// CLI binary
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn cli_without_args_prints_usage_and_exits_nonzero() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_playlist-cli"))
        .output()
        .expect("failed to run playlist-cli binary");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage:"), "stderr was: {stderr}");
}
