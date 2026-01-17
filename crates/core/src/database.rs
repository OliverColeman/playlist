use rand::Rng;
use std::env::VarError;

#[derive(Debug)]
pub enum ServerError {
    DatabaseError(mongodb::error::Error),
    ConfigError(VarError),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::DatabaseError(e) => write!(f, "Database error: {}", e),
            ServerError::ConfigError(e) => write!(f, "Config error: {}", e),
        }
    }
}

impl std::error::Error for ServerError {}

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

pub async fn get_database() -> Result<mongodb::Database, ServerError> {
    let config = crate::config::Config::from_env()?;
    let client = mongodb::Client::with_uri_str(&config.db.connection_string).await?;
    let database = client.database(&config.db.db_name);
    Ok(database)
}

pub fn generate_id() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const ID_LEN: usize = 17;
    let mut rng = rand::rng();
    (0..ID_LEN)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
