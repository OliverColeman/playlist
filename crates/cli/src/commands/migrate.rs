use mongodb::Database;

pub async fn run(database: Database) -> Result<(), Box<dyn std::error::Error>> {
    update_collection_and_field_names(database).await
}

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
        
        // TODO: Add actual migration logic here
        // For example:
        // - Copy documents from old collection to new collection
        // - Update field names within documents
        // - Delete old collection
    }
    
    Ok(())
}

fn camel_to_snake_case(name: &str) -> String {
    let mut snake_case = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                snake_case.push('_');
            }
            for lower_c in c.to_lowercase() {
                snake_case.push(lower_c);
            }
        } else {
            snake_case.push(c);
        }
    }
    snake_case
}
