use std::env::VarError;

pub struct MongodbConfig {
    pub connection_string: String,
    pub db_name: String,
}

pub struct Config {
    pub db: MongodbConfig,
}

impl Config {
    pub fn from_env() -> Result<Self, VarError> {
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
