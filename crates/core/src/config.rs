use std::env::VarError;

#[derive(Debug, Clone)]
pub struct MongodbConfig {
    pub connection_string: String,
    pub db_name: String,
}

#[derive(Debug, Clone)]
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

// This module is only compiled with the "server" feature (see lib.rs).
#[cfg(test)]
mod tests {
    use super::*;

    /// Captures the current values of a set of environment variables and restores them
    /// on drop, so the environment is put back even when an assertion panics mid-test
    /// (a leaked env var would cascade confusing failures into later tests in the same
    /// `--include-ignored --test-threads=1` run).
    struct EnvRestoreGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl EnvRestoreGuard {
        fn capture(keys: &[&'static str]) -> Self {
            Self {
                saved: keys.iter().map(|&k| (k, std::env::var(k).ok())).collect(),
            }
        }
    }

    impl Drop for EnvRestoreGuard {
        fn drop(&mut self) {
            // SAFETY: only used by tests that are run with --test-threads=1 (they are
            // ignored by default), so no other thread accesses the process environment
            // concurrently.
            for (key, value) in &self.saved {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }

    /// Environment variables are process-global, so both the success and the error case
    /// are exercised sequentially in this single test, and the test is ignored by
    /// default so the parallel test runner can never race on the environment.
    #[test]
    #[ignore = "mutates process env; run via dev/test scripts with --test-threads=1"]
    fn from_env_reads_vars_and_errors_when_one_is_missing() {
        // Restores the pre-test environment on drop, even if an assertion panics.
        let _env_guard = EnvRestoreGuard::capture(&["DB_CONNECTION_STRING", "DB_NAME"]);

        // SAFETY: this test is only run with --test-threads=1 (it is ignored by
        // default), so no other thread accesses the process environment concurrently.
        unsafe {
            std::env::set_var("DB_CONNECTION_STRING", "mongodb://config-test-host:27017");
            std::env::set_var("DB_NAME", "config_test_db");
        }
        let config = Config::from_env().expect("from_env must succeed when both vars are set");
        assert_eq!(
            config.db.connection_string,
            "mongodb://config-test-host:27017"
        );
        assert_eq!(config.db.db_name, "config_test_db");

        // SAFETY: see above; single-threaded test execution.
        unsafe {
            std::env::remove_var("DB_CONNECTION_STRING");
        }
        let err = Config::from_env()
            .expect_err("from_env must fail when DB_CONNECTION_STRING is missing");
        assert!(
            matches!(err, VarError::NotPresent),
            "unexpected error: {err:?}"
        );

        // SAFETY: see above; single-threaded test execution.
        unsafe {
            std::env::set_var("DB_CONNECTION_STRING", "mongodb://config-test-host:27017");
            std::env::remove_var("DB_NAME");
        }
        let err = Config::from_env().expect_err("from_env must fail when DB_NAME is missing");
        assert!(
            matches!(err, VarError::NotPresent),
            "unexpected error: {err:?}"
        );
        // _env_guard restores DB_CONNECTION_STRING and DB_NAME here.
    }
}
