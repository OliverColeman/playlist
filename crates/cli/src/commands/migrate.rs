use futures::TryStreamExt;
use mongodb::Database;

pub async fn run(database: Database) -> Result<(), Box<dyn std::error::Error>> {
    update_collection_and_field_names(database).await
}

static FIELDS_TO_REMOVE: [&str; 2] = ["appearsInPlayLists", "appearsInPlayListGroups"];

pub async fn update_collection_and_field_names(
    database: Database,
) -> Result<(), Box<dyn std::error::Error>> {
    let collection_names = vec![
        "Album",
        "Artist",
        "Compiler",
        "LinkedTrack",
        "PlayList",
        "Track",
    ];

    for old_collection_name in collection_names {
        let old_collection = database.collection::<mongodb::bson::Document>(old_collection_name);
        let new_collection_name = camel_to_snake_case(old_collection_name);
        let new_collection = database.collection::<mongodb::bson::Document>(&new_collection_name);
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

            new_collection.insert_one(new_doc).await?;
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
