use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SyncError>;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<hypr_supabase_auth::Error> for SyncError {
    fn from(err: hypr_supabase_auth::Error) -> Self {
        Self::Auth(err.to_string())
    }
}

impl IntoResponse for SyncError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Auth(message) => (StatusCode::UNAUTHORIZED, "unauthorized", message),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                message,
            ),
        };

        hypr_api_error::error_response(status, code, &message)
    }
}
