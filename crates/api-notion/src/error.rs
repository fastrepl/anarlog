use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, NotionError>;

#[derive(Debug, Error)]
pub enum NotionError {
    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Notion error: {0}")]
    Notion(String),

    #[error("Internal error: {0}")]
    #[allow(dead_code)]
    Internal(String),

    #[error(transparent)]
    NangoConnection(#[from] anlg_api_nango::NangoConnectionError),
}

impl IntoResponse for NotionError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Notion(message) => (StatusCode::INTERNAL_SERVER_ERROR, "notion_error", message),
            Self::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                message,
            ),
            Self::NangoConnection(err) => return err.into_response(),
        };

        anlg_api_error::error_response(status, code, &message)
    }
}
