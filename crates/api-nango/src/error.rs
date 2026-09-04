use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, NangoError>;

#[derive(Debug, Error)]
pub enum NangoError {
    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Nango error: {0}")]
    Nango(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    #[allow(dead_code)]
    Internal(String),
}

impl From<anlg_nango::Error> for NangoError {
    fn from(_err: anlg_nango::Error) -> Self {
        Self::Nango("connector provider request failed".to_string())
    }
}

impl IntoResponse for NangoError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Auth(message) => (StatusCode::UNAUTHORIZED, "unauthorized", message),
            Self::Forbidden(message) => (StatusCode::FORBIDDEN, "forbidden", message),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Nango(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "nango_error",
                "Connector request failed".to_string(),
            ),
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                "Internal server error".to_string(),
            ),
        };

        anlg_api_error::error_response(status, code, &message)
    }
}
