use std::collections::HashSet;

use futures::TryStreamExt;
use mongodb::Database;

pub async fn run(database: Database) -> Result<(), Box<dyn std::error::Error>> {
    update_collection_and_field_names(database.clone()).await?;
    build_linked_tracks(database).await?;
    Ok(())
}

static FIELDS_TO_REMOVE: [&str; 4] = [
    "appearsInPlayLists",
    "appearsInPlayListGroups",
    "name_normalised",
    "name_normalised_strong",
];

pub async fn update_collection_and_field_names(
    database: Database,
) -> Result<(), Box<dyn std::error::Error>> {
    use playlist_core::{normalise_name, normalise_name_strong};

    let collection_names = vec!["Album", "Artist", "Compiler", "PlayList", "Track"];

    for old_collection_name in collection_names {
        let old_collection = database.collection::<mongodb::bson::Document>(old_collection_name);
        let new_collection_name = camel_to_snake_case(old_collection_name);
        let new_collection = database.collection::<mongodb::bson::Document>(&new_collection_name);

        // Remove pre-existing docs if any
        new_collection.delete_many(mongodb::bson::doc! {}).await?;

        println!(
            "Updating collection: {} to {}",
            old_collection_name, new_collection_name
        );

        for doc in old_collection
            .find(mongodb::bson::doc! {})
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
        {
            let mut new_doc = mongodb::bson::Document::new();

            for (key, value) in doc.iter() {
                if !FIELDS_TO_REMOVE.contains(&key.as_str()) {
                    let new_key = camel_to_snake_case(key);
                    new_doc.insert(new_key, value.clone());
                }
            }

            // Add/update normalised name fields using current normalisation functions
            new_doc.insert(
                "name_normalised",
                normalise_name(doc.get_str("name").unwrap_or("")),
            );
            new_doc.insert(
                "name_normalised_strong",
                normalise_name_strong(doc.get_str("name").unwrap_or("")),
            );

            new_collection.insert_one(new_doc).await?;
        }
    }

    Ok(())
}

struct LinkedTrackIntermediate {
    name_normalised_strong: String,
    artist_ids: HashSet<String>,
    track_ids: HashSet<String>,
}

pub async fn build_linked_tracks(database: Database) -> Result<(), Box<dyn std::error::Error>> {
    println!("Building linked tracks...");
    use playlist_core::models::server::load_items;
    use playlist_core::models::track::Track;

    let mut linked_tracks_by_name: std::collections::HashMap<String, Vec<LinkedTrackIntermediate>> =
        std::collections::HashMap::new();

    let tracks = load_items::<Track>(mongodb::bson::doc! {}).await?;
    for track in tracks {
        let name_normalised_strong = track.name_normalised_strong.clone();

        let track_artist_ids: std::collections::HashSet<String> =
            track.artist_ids.iter().cloned().collect();

        let linked_tracks = linked_tracks_by_name
            .entry(name_normalised_strong.clone())
            .or_default();

        // If an existing linked track exists for this (normalised) name with at least one of the track's artists
        let existing_linked_track = linked_tracks
            .iter_mut()
            .find(|lt| !track_artist_ids.is_disjoint(&lt.artist_ids));
        match existing_linked_track {
            Some(lt) => {
                // Add track ID to existing linked track
                lt.track_ids.insert(track.id.clone());
                // Add artist IDs to existing linked track
                for artist_id in &track.artist_ids {
                    lt.artist_ids.insert(artist_id.clone());
                }
            }
            None => {
                // Create new linked track
                linked_tracks.push(LinkedTrackIntermediate {
                    name_normalised_strong: name_normalised_strong.clone(),
                    artist_ids: track.artist_ids.iter().cloned().collect(),
                    track_ids: vec![track.id.clone()].into_iter().collect(),
                });
            }
        }
    }

    let lt_collection =
        database.collection::<playlist_core::models::track::LinkedTrack>("linked_track");
    // Remove pre-existing docs if any
    lt_collection.delete_many(mongodb::bson::doc! {}).await?;

    println!("Inserting linked tracks: {}", linked_tracks_by_name.len());

    for linked_tracks in linked_tracks_by_name.values() {
        for lt in linked_tracks {
            if lt.track_ids.len() < 2 {
                continue;
            }
            let new_lt = playlist_core::models::track::LinkedTrack {
                id: playlist_core::database::generate_id(),
                track_name_normalised_strong: lt.name_normalised_strong.clone(),
                track_ids: lt.track_ids.iter().cloned().collect(),
                artist_ids: lt.artist_ids.iter().cloned().collect(),
            };

            lt_collection.insert_one(&new_lt).await?;
        }
    }

    Ok(())
}

fn camel_to_snake_case(text: &str) -> String {
    // Special cases
    let text = text.replace("PlayList", "Playlist").replace("URL", "Url");

    let mut buffer = String::with_capacity(text.len() + text.len() / 2);

    let mut text = text.chars();

    if let Some(first) = text.next() {
        let mut n2: Option<(bool, char)> = None;
        let mut n1: (bool, char) = (first.is_lowercase(), first);

        for c in text {
            let prev_n1 = n1.clone();

            let n3 = n2;
            n2 = Some(n1);
            n1 = (c.is_lowercase(), c);

            // insert underscore if acronym at beginning
            // ABc -> a_bc
            if let Some((false, c3)) = n3
                && let Some((false, c2)) = n2
                && n1.0
                && c3.is_uppercase()
                && c2.is_uppercase()
            {
                buffer.push('_');
            }

            buffer.push_str(&prev_n1.1.to_lowercase().to_string());

            // insert underscore before next word
            // abC -> ab_c
            if let Some((true, _)) = n2
                && n1.1.is_uppercase()
            {
                buffer.push('_');
            }
        }

        buffer.push_str(&n1.1.to_lowercase().to_string());
    }

    buffer
}
