use dioxus::prelude::*;
use playlist_core::ServerError as CoreServerError;

#[derive(Debug)]
pub enum ServerError {
    CoreError(CoreServerError),
}

impl From<CoreServerError> for ServerError {
    fn from(err: CoreServerError) -> Self {
        ServerError::CoreError(err)
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

// This module (and therefore this test module) is only compiled with the "server" feature, so
// these tests run under `cargo test -p playlist-web --features server`.
#[cfg(test)]
mod tests {
    use super::*;
    use std::env::VarError;

    #[test]
    fn core_server_error_wraps_into_server_error() {
        let err: ServerError = CoreServerError::ConfigError(VarError::NotPresent).into();
        assert!(matches!(
            err,
            ServerError::CoreError(CoreServerError::ConfigError(VarError::NotPresent))
        ));
    }

    #[test]
    fn server_error_converts_to_500_server_fn_error() {
        let err = ServerError::CoreError(CoreServerError::ConfigError(VarError::NotPresent));

        // The details payload is the Debug rendering of the ServerError, which includes the
        // Debug rendering of the wrapped core error.
        let debug_repr = format!("{:?}", err);
        let wrapped_debug = format!("{:?}", CoreServerError::ConfigError(VarError::NotPresent));
        assert!(debug_repr.contains(&wrapped_debug));
        assert_eq!(debug_repr, "CoreError(ConfigError(NotPresent))");

        let server_fn_err: ServerFnError = err.into();
        assert_eq!(
            server_fn_err,
            ServerFnError::ServerError {
                message: "Internal server error".to_string(),
                code: 500,
                details: Some(serde_json::Value::String(debug_repr)),
            }
        );
    }
}
