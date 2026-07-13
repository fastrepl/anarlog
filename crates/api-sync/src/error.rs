use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SyncError>;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("CloudSync credential service is unavailable")]
    Upstream,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for SyncError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Upstream => (
                StatusCode::BAD_GATEWAY,
                "cloudsync_credential_service_unavailable",
                "CloudSync credential service is unavailable".to_string(),
            ),
            Self::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                message,
            ),
        };

        hypr_api_error::error_response(status, code, &message)
    }
}
