pub mod app;
pub mod config;
pub mod music_data;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use server_fn::codec::JsonEncoding;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}

#[cfg(feature = "ssr")]
mod ssr {
    use super::*;
    use mongodb;
    use std::env::VarError;

    #[derive(Debug)]
    pub enum ServerError {
        MongoError(mongodb::error::Error),
        ConfigError(String),
        NotFound(String),
    }

    impl From<mongodb::error::Error> for ServerError {
        fn from(err: mongodb::error::Error) -> Self {
            ServerError::MongoError(err)
        }
    }

    impl From<VarError> for ServerError {
        fn from(err: VarError) -> Self {
            ServerError::ConfigError(err.to_string())
        }
    }

    impl From<ServerError> for AppError {
        fn from(err: ServerError) -> Self {
            match err {
                ServerError::MongoError(e) => AppError::ServerError(e.to_string()),
                ServerError::ConfigError(msg) => AppError::ServerError(msg),
                ServerError::NotFound(msg) => AppError::ServerError(msg),
            }
        }
    }

    use leptos::logging::log;
    pub async fn get_database() -> Result<mongodb::Database, crate::ssr::ServerError> {
        async {
            let config = crate::config::Config::from_env()?;
            let client = mongodb::Client::with_uri_str(&config.db.connection_string).await?;
            let database = client.database(&config.db.db_name);
            Ok(database)
        }
        .await
        .map_err(|e| {
            log!("Error connecting to database: {:?}", e);
            e
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

/// Formats a duration in seconds to "[HH:]MM:SS" format.
pub fn format_duration(seconds: f64) -> String {
    let total_seconds = seconds.round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    // If hours is 0, omit it
    if hours == 0 {
        return format!("{:02}:{:02}", minutes, seconds);
    }
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

// Formats a unix timestamp (in seconds) to "YYYY-MM-DD" format.
pub fn format_date(timestamp: Option<f64>) -> String {
    match timestamp {
        Some(ts) => chrono::DateTime::from_timestamp(ts as i64, 0)
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d")
            .to_string(),
        None => "Unknown".to_string(),
    }
}
