pub mod config;
use dioxus::prelude::*;
use std::env::VarError;

#[derive(Debug)]
pub enum ServerError {
    DatabaseError(mongodb::error::Error),
    ConfigError(VarError),
}

impl From<mongodb::error::Error> for ServerError {
    fn from(err: mongodb::error::Error) -> Self {
        ServerError::DatabaseError(err)
    }
}

impl From<VarError> for ServerError {
    fn from(err: VarError) -> Self {
        ServerError::ConfigError(err)
    }
}

impl From<ServerError> for ServerFnError {
    fn from(err: ServerError) -> Self {
        ServerFnError::ServerError {
            message: "Internal server error".to_string(),
            code: 500,
            details: Some(serde_json::json!(format!("{:?}", err))),
        }
    }
}

pub async fn get_database() -> Result<mongodb::Database, ServerError> {
    let config = config::Config::from_env()?;
    let client = mongodb::Client::with_uri_str(&config.db.connection_string).await?;
    let database = client.database(&config.db.db_name);
    Ok(database)
}
