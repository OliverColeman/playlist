pub mod app;

use std::env::VarError;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use server_fn::codec::JsonEncoding;

#[cfg(feature = "ssr")]
use leptos::logging::log;
#[cfg(feature = "ssr")]
use mongodb;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}

#[cfg(feature = "ssr")]
#[derive(Debug)]
pub enum ServerError {
    MongoError(mongodb::error::Error),
    ConfigError(String),
}
#[cfg(feature = "ssr")]
impl From<mongodb::error::Error> for ServerError {
    fn from(err: mongodb::error::Error) -> Self {
        ServerError::MongoError(err)
    }
}
#[cfg(feature = "ssr")]
impl From<VarError> for ServerError {
    fn from(err: VarError) -> Self {
        ServerError::ConfigError(err.to_string())
    }
}

#[cfg(feature = "ssr")]
pub struct MongodbConfig {
    pub connection_string: String,
    pub db_name: String,
}

#[cfg(feature = "ssr")]
pub struct Config {
    pub db: MongodbConfig,
}

#[cfg(feature = "ssr")]
impl Config {
    pub fn from_env() -> Result<Self, ServerError> {
        let db_connection_string = std::env::var("DB_CONNECTION_STRING")?;
        let db_name = std::env::var("DB_NAME")?;

        Ok(Config {
            db: MongodbConfig {
                connection_string: db_connection_string,
                db_name,
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum AppError {
    ServerFnError(ServerFnErrorErr),
    ServerError(String),
}

impl FromServerFnError for AppError {
    type Encoder = JsonEncoding;

    fn from_server_fn_error(value: ServerFnErrorErr) -> Self {
        AppError::ServerFnError(value)
    }
}
#[cfg(feature = "ssr")]
impl From<ServerError> for AppError {
    fn from(err: ServerError) -> Self {
        match err {
            ServerError::MongoError(e) => AppError::ServerError(e.to_string()),
            ServerError::ConfigError(msg) => AppError::ServerError(msg),
        }
    }
}

#[server]
pub async fn load_playlist() -> Result<String, AppError> {
    let result: Result<String, ServerError> = async {
        let config = Config::from_env()?;
        log!("Connecting to mongodb at {}", config.db.connection_string);
        let client = mongodb::Client::with_uri_str(&config.db.connection_string).await?;
        let database = client.database(&config.db.db_name);
        let collection: mongodb::Collection<mongodb::bson::Document> =
            database.collection("PlayList");
        let playlist = collection
            .find_one(mongodb::bson::doc! { "name": "JD 235" })
            .await?;
        let playlist_name = playlist
            .and_then(|doc| doc.get_str("name").ok().map(|s| s.to_string()))
            .unwrap_or("Playlist not found".to_string());
        Ok(playlist_name)
    }
    .await;

    match result {
        Ok(name) => Ok(name),
        Err(e) => {
            log!("Error loading playlist: {:?}", e);
            Err(e.into())
        }
    }

    // result.map_err(|e| e.into())

    // match insert_user_into_db(&name, &email).await {
    //     Ok(user) => Ok(user),
    //     Err(e) => Err(AppError::DbError(e.to_string())),
    // }
}
