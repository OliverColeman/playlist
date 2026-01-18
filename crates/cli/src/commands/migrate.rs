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
    use playlist_core::models::album::Album;
    use playlist_core::models::artist::Artist;
    use playlist_core::models::compiler::Compiler;
    use playlist_core::models::playlist::PlayList;
    use playlist_core::models::track::Track;

    use playlist_core::models::server::load_music_items;
    use playlist_core::{
        generate_double_metaphone_codes, generate_n_grams, normalise_name, normalise_name_strong,
    };

    // This ordering is important: some computed fields from earlier collections are used in later collections
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

            // Add/update normalised name fields using current implementations
            let name_normalised = normalise_name(doc.get_str("name").unwrap_or(""));
            new_doc.insert("name_normalised", name_normalised.clone());
            new_doc.insert(
                "name_normalised_strong",
                normalise_name_strong(doc.get_str("name").unwrap_or("")),
            );
            // Add/update search fields
            new_doc.insert(
                "name_double_metaphone_codes",
                generate_double_metaphone_codes(name_normalised.as_str()),
            );
            new_doc.insert("name_n_grams", generate_n_grams(name_normalised.as_str()));

            if new_collection_name == "playlist" {
                // For playlists, add search fields for all the compiler names
                let mut double_metaphone_codes = HashSet::<String>::new();
                let mut n_grams = HashSet::<String>::new();
                let compiler_ids = new_doc.get_array("compiler_ids").unwrap_or(&vec![]).clone();
                let compiler_names_normalised = load_music_items::<Compiler>(mongodb::bson::doc! {
                    "_id": { "$in": compiler_ids }
                })
                .await
                .unwrap()
                .into_iter()
                .map(|c| c.name_normalised)
                .collect::<Vec<String>>();
                for compiler_name_normalised in compiler_names_normalised {
                    for code in generate_double_metaphone_codes(compiler_name_normalised.as_str()) {
                        double_metaphone_codes.insert(code);
                    }
                    for n_gram in generate_n_grams(compiler_name_normalised.as_str()) {
                        n_grams.insert(n_gram);
                    }
                }
                new_doc.insert(
                    "compiler_names_double_metaphone_codes",
                    double_metaphone_codes.into_iter().collect::<Vec<String>>(),
                );
                new_doc.insert(
                    "compiler_names_n_grams",
                    n_grams.into_iter().collect::<Vec<String>>(),
                );
            }

            if new_collection_name == "track" {
                // For tracks, add search fields for all the artist names
                let mut double_metaphone_codes = HashSet::<String>::new();
                let mut n_grams = HashSet::<String>::new();
                let artist_names_normalised = load_music_items::<Artist>(mongodb::bson::doc! {
                    "_id": new_doc.get_array("artist_ids").unwrap_or(&vec![]).clone()
                })
                .await?
                .into_iter()
                .map(|a| a.name_normalised)
                .collect::<Vec<String>>();
                for artist_name_normalised in artist_names_normalised {
                    for code in generate_double_metaphone_codes(artist_name_normalised.as_str()) {
                        double_metaphone_codes.insert(code);
                    }
                    for n_gram in generate_n_grams(artist_name_normalised.as_str()) {
                        n_grams.insert(n_gram);
                    }
                }
                new_doc.insert(
                    "artist_names_double_metaphone_codes",
                    double_metaphone_codes.into_iter().collect::<Vec<String>>(),
                );
                new_doc.insert(
                    "artist_names_n_grams",
                    n_grams.into_iter().collect::<Vec<String>>(),
                );
            }

            match new_collection_name.as_str() {
                "album" => {
                    let typed_doc: Album = mongodb::bson::from_document(new_doc.clone())?;
                    let typed_collection = database.collection::<Album>(&new_collection_name);
                    typed_collection.insert_one(typed_doc).await?;
                }
                "artist" => {
                    let typed_doc: Artist = mongodb::bson::from_document(new_doc.clone())?;
                    let typed_collection = database.collection::<Artist>(&new_collection_name);
                    typed_collection.insert_one(typed_doc).await?;
                }
                "compiler" => {
                    let typed_doc: Compiler = mongodb::bson::from_document(new_doc.clone())?;
                    let typed_collection = database.collection::<Compiler>(&new_collection_name);
                    typed_collection.insert_one(typed_doc).await?;
                }
                "track" => {
                    let typed_doc: Track = mongodb::bson::from_document(new_doc.clone())?;
                    let typed_collection = database.collection::<Track>(&new_collection_name);
                    typed_collection.insert_one(typed_doc).await?;
                }
                "playlist" => {
                    let typed_doc: PlayList = mongodb::bson::from_document(new_doc.clone())?;
                    let typed_collection = database.collection::<PlayList>(&new_collection_name);
                    typed_collection.insert_one(typed_doc).await?;
                }
                _ => {
                    println!("Warning: Unknown collection name: {}.", new_collection_name);
                }
            }
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
    use playlist_core::models::server::load_music_items;
    use playlist_core::models::track::Track;

    let mut linked_tracks_by_name: std::collections::HashMap<String, Vec<LinkedTrackIntermediate>> =
        std::collections::HashMap::new();

    let tracks = load_music_items::<Track>(mongodb::bson::doc! {}).await?;
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
