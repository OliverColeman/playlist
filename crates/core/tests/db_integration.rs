//! Integration tests against a real MongoDB instance.
//!
//! All tests are ignored by default and expect `DB_CONNECTION_STRING` and `DB_NAME`
//! (=`playlist_test_core`) to be set in the environment. Run them with:
//!
//! ```sh
//! DB_CONNECTION_STRING="mongodb://localhost:27017" DB_NAME="playlist_test_core" \
//!     cargo test -p playlist-core --features server --test db_integration -- \
//!     --include-ignored --test-threads=1
//! ```

#![cfg(feature = "server")]

use mongodb::bson::doc;
use playlist_core::models::MusicItemBase;
use playlist_core::models::compiler::Compiler;
use playlist_core::models::track::{LinkedTrack, Track, load_linked_tracks};

/// `playlist_core::get_database` caches a process-global `mongodb::Client` whose
/// background monitor tasks are spawned on the tokio runtime that first created it.
/// `#[tokio::test]` gives every test its own runtime, so once the first test's runtime
/// is dropped the cached client would be unusable in later tests (immediate
/// "server selection timeout" errors). To keep the cached client alive for the whole
/// test binary, all database work runs on this single shared runtime, which is stored
/// in a static and therefore never dropped.
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

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set for integration tests"))
}

/// Drops the test database (via a raw client, independent of the crate's cached client)
/// so every test starts from a clean state, and returns a handle to it.
async fn reset_test_database() -> mongodb::Database {
    let connection_string = required_env("DB_CONNECTION_STRING");
    let db_name = required_env("DB_NAME");
    let client = mongodb::Client::with_uri_str(&connection_string)
        .await
        .expect("failed to create raw MongoDB client");
    let database = client.database(&db_name);
    database.drop().await.expect("failed to drop test database");
    database
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
        search_terms: vec![name.to_lowercase()],
        search_double_metaphone_codes: vec![],
        search_n_grams: vec![],
        artist_ids: artist_ids.iter().map(|s| s.to_string()).collect(),
        album_id: None,
        duration: Some(180.25),
    }
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn get_database_returns_a_usable_handle() {
    run_on_shared_runtime(async {
        reset_test_database().await;

        let database = playlist_core::get_database()
            .await
            .expect("get_database failed");
        assert_eq!(database.name(), required_env("DB_NAME"));

        // The handle must be usable for round-tripping documents.
        let collection = database.collection::<mongodb::bson::Document>("smoke_test");
        collection
            .insert_one(doc! { "_id": "smoke1", "value": 42 })
            .await
            .expect("insert failed");
        let found = collection
            .find_one(doc! { "_id": "smoke1" })
            .await
            .expect("find_one failed")
            .expect("inserted document not found");
        assert_eq!(found.get_i32("value").unwrap(), 42);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn load_music_items_round_trips_typed_documents() {
    run_on_shared_runtime(async {
        let database = reset_test_database().await;

        let collection = database.collection::<Track>(Track::collection_name());
        let track_one = make_track("t1", "Song One", &["a1"]);
        let track_two = make_track("t2", "Song Two", &["a1", "a2"]);
        collection
            .insert_many([track_one, track_two])
            .await
            .expect("insert_many failed");

        // Empty filter loads everything.
        let mut all: Vec<Track> = playlist_core::models::server::load_music_items(doc! {})
            .await
            .expect("load_music_items with empty filter failed");
        all.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, "t1");
        assert_eq!(all[0].name, "Song One");
        assert_eq!(all[0].name_normalised, "song one");
        assert_eq!(all[0].artist_ids, ["a1"]);
        assert_eq!(all[0].duration, Some(180.25));
        assert_eq!(all[0].album_id, None);
        assert_eq!(all[1].id, "t2");
        assert_eq!(all[1].artist_ids, ["a1", "a2"]);

        // A bson filter restricts the result.
        let filtered: Vec<Track> =
            playlist_core::models::server::load_music_items(doc! { "name_normalised": "song two" })
                .await
                .expect("load_music_items with filter failed");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "t2");
        assert_eq!(filtered[0].name, "Song Two");
        assert_eq!(filtered[0].search_terms, ["song two"]);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn create_indexes_succeeds_and_is_idempotent() {
    run_on_shared_runtime(async {
        let database = reset_test_database().await;

        playlist_core::models::server::create_indexes::<Compiler>()
            .await
            .expect("first create_indexes call failed");
        // Creating the same index again must not fail.
        playlist_core::models::server::create_indexes::<Compiler>()
            .await
            .expect("second create_indexes call failed (not idempotent)");

        let collection = database.collection::<Compiler>(Compiler::collection_name());
        let mut index_keys = Vec::new();
        let mut cursor = collection
            .list_indexes()
            .await
            .expect("list_indexes failed");
        while cursor.advance().await.expect("cursor advance failed") {
            let index = cursor
                .deserialize_current()
                .expect("failed to deserialize index model");
            index_keys.push(index.keys);
        }

        assert!(
            index_keys.contains(&doc! { "name_normalised": 1 }),
            "expected an index on name_normalised, got: {index_keys:?}"
        );
        // Exactly the default _id index plus one name_normalised index: calling
        // create_indexes twice must not create a duplicate.
        assert_eq!(index_keys.len(), 2, "unexpected indexes: {index_keys:?}");
    })
    .await;
}

#[tokio::test]
#[ignore = "requires MongoDB; run via dev/test/run_integration.sh"]
async fn load_linked_tracks_queries_by_track_id_membership() {
    run_on_shared_runtime(async {
        let database = reset_test_database().await;

        let collection = database.collection::<LinkedTrack>("linked_track");
        collection
            .insert_many([
                LinkedTrack {
                    id: "lt1".to_string(),
                    track_name_normalised_strong: "song one".to_string(),
                    track_ids: vec!["t1".to_string(), "t2".to_string()],
                    artist_ids: vec!["a1".to_string()],
                },
                LinkedTrack {
                    id: "lt2".to_string(),
                    track_name_normalised_strong: "song two".to_string(),
                    track_ids: vec!["t3".to_string()],
                    artist_ids: vec!["a2".to_string()],
                },
            ])
            .await
            .expect("insert_many failed");

        // Membership query: a track id inside the track_ids array matches its group.
        let hits = load_linked_tracks(doc! { "track_ids": "t2" })
            .await
            .expect("load_linked_tracks failed");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "lt1");
        assert_eq!(hits[0].track_name_normalised_strong, "song one");
        assert_eq!(hits[0].track_ids, ["t1", "t2"]);
        assert_eq!(hits[0].artist_ids, ["a1"]);

        // A track id that belongs to no group matches nothing.
        let misses = load_linked_tracks(doc! { "track_ids": "unknown" })
            .await
            .expect("load_linked_tracks failed");
        assert!(misses.is_empty());

        // An empty query returns all linked-track documents.
        let mut all = load_linked_tracks(doc! {})
            .await
            .expect("load_linked_tracks failed");
        all.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].id, "lt2");
        assert_eq!(all[1].track_ids, ["t3"]);

        // load_linked_tracks also ensures the track_ids index exists.
        let index_names = collection
            .list_index_names()
            .await
            .expect("list_index_names failed");
        assert!(
            index_names.contains(&"track_ids_1".to_string()),
            "expected track_ids_1 index, got: {index_names:?}"
        );
    })
    .await;
}
