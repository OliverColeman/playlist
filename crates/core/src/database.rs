use crate::config::Config;
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

// Global cached MongoDB client - initialized once on first use
static DB_CLIENT: tokio::sync::OnceCell<mongodb::Client> = tokio::sync::OnceCell::const_new();

/// Initialize and cache the MongoDB client. This is called automatically by get_database().
async fn get_or_init_client(config: Config) -> Result<&'static mongodb::Client, ServerError> {
    DB_CLIENT
        .get_or_try_init(|| async {
            use mongodb::options::ClientOptions;
            use std::time::Duration;

            // Parse connection string and configure client options
            let mut client_options = ClientOptions::parse(&config.db.connection_string).await?;

            // Enable automatic retries for reads and writes
            client_options.retry_reads = Some(true);
            client_options.retry_writes = Some(true);

            // Configure timeouts
            client_options.server_selection_timeout = Some(Duration::from_secs(30));
            client_options.connect_timeout = Some(Duration::from_secs(10));

            // Create client with configured options
            let client = mongodb::Client::with_options(client_options)?;

            // Ping the database to verify connection
            client
                .database("admin")
                .run_command(mongodb::bson::doc! { "ping": 1 })
                .await
                .map_err(|e| {
                    eprintln!("Failed to connect to MongoDB: {}", e);
                    eprintln!(
                        "Make sure MongoDB is running and accessible at: {}",
                        &config.db.connection_string
                    );
                    ServerError::DatabaseError(e)
                })?;

            println!("MongoDB client initialized successfully");
            Ok(client)
        })
        .await
}

/// Get a database handle. The underlying client connection is cached and reused.
pub async fn get_database() -> Result<mongodb::Database, ServerError> {
    let config = Config::from_env()?;
    let client = get_or_init_client(config.clone()).await?;
    Ok(client.database(&config.db.db_name))
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
