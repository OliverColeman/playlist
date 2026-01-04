use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(Compiler, "Compiler", {
    // No additional fields
});

#[get("/api/compilers")]
pub async fn load_compilers() -> Result<Vec<Compiler>, ServerFnError> {
    use crate::music_data::server::load_items;
    use mongodb::bson;

    let result = load_items::<Compiler>(bson::doc! {}).await;

    if let Err(ref e) = result {
        tracing::info!("Error loading compilers: {:?}", e);
    }
    if let Ok(ref compilers) = result {
        tracing::info!("Loaded {} compilers", compilers.len());
    }
    // simulate a longer load time for demonstration purposes
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    Ok(result?)
}
