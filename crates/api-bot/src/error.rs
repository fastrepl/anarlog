use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, BotError>;

#[derive(Debug, Error)]
pub enum BotError {
    #[error("Invalid request: {0}")]
    #[allow(dead_code)]
    BadRequest(String),

    #[error("Recall API error: {0}")]
    Recall(#[from] hypr_recall::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for BotError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            Self::Recall(err) => (StatusCode::BAD_GATEWAY, "recall_error", err.to_string()),
            Self::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                msg,
            ),
        };

        hypr_api_error::error_response(status, code, &message)
    }
}
