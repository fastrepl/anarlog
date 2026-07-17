use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Authentication error: {0}")]
    #[allow(dead_code)]
    Auth(String),

    #[error("Invalid request: {0}")]
    #[allow(dead_code)]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    NangoConnection(#[from] hypr_api_nango::NangoConnectionError),
}

impl IntoResponse for StorageError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Auth(message) => (StatusCode::UNAUTHORIZED, "unauthorized", message),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                message,
            ),
            Self::NangoConnection(err) => return err.into_response(),
        };

        hypr_api_error::error_response(status, code, &message)
    }
}
