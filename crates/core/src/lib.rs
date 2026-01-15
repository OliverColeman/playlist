pub mod models;

#[cfg(feature = "server")]
pub mod config;

#[cfg(feature = "server")]
pub mod database;

#[cfg(feature = "server")]
pub use config::Config;

#[cfg(feature = "server")]
pub use database::{ServerError, get_database};
