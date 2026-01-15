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
