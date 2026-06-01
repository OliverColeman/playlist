use mongodb::Database;
use mongodb::bson::doc;
use playlist_core::models::{MusicItemBase, compiler::Compiler};
use playlist_core::{normalise_name, normalise_name_strong};

use crate::commands::import_playlist::build_search_fields;

/// Set the name of an existing Compiler, keeping its normalised-name and search-index
/// fields in sync with the new name.
pub async fn set_compiler_name(
    database: Database,
    compiler_id: &str,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let collection = database.collection::<Compiler>(Compiler::collection_name());

    let Some(mut compiler) = collection.find_one(doc! { "_id": compiler_id }).await? else {
        return Err(format!("No compiler found with id \"{}\"", compiler_id).into());
    };

    let previous_name = compiler.name.clone();

    compiler.name = name.to_string();
    compiler.name_normalised = normalise_name(name);
    compiler.name_normalised_strong = normalise_name_strong(name);

    let (search_terms, search_double_metaphone_codes, search_n_grams) =
        build_search_fields(&[normalise_name(name)]);
    compiler.search_terms = search_terms;
    compiler.search_double_metaphone_codes = search_double_metaphone_codes;
    compiler.search_n_grams = search_n_grams;

    collection
        .replace_one(doc! { "_id": compiler_id }, &compiler)
        .await?;

    println!(
        "Renamed compiler {} from \"{}\" to \"{}\".",
        compiler_id, previous_name, name
    );

    Ok(())
}
