#[cfg(feature = "ssr")]
use leptos::logging::log;
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use mongodb::bson;
use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(Compiler, {
    // No additional fields
});

#[server]
pub async fn load_compilers() -> Result<Vec<Compiler>, crate::AppError> {
    let result: Result<Vec<Compiler>, crate::ssr::ServerError> = async {
        use futures::stream::TryStreamExt;
        let database = crate::ssr::get_database().await?;
        let collection: mongodb::Collection<Compiler> = database.collection("Compiler");
        let cursor = collection.find(bson::doc! {}).sort(bson::doc! {"name": 1}).await?;
        let compilers = cursor.try_collect().await?;
        Ok(compilers)
    }
    .await;
    // Log error if any
    if let Err(ref e) = result {
        log!("Error loading compilers: {:?}", e);
    }
    // Log length of compilers loaded
    if let Ok(ref compilers) = result {
        log!("Loaded {} compilers", compilers.len());
    }
    result.map_err(|e| crate::AppError::from(e))
}

impl PartialEq for Compiler {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
