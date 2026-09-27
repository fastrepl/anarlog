use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CrmError>;

#[derive(Debug, Error)]
pub enum CrmError {
    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("CRM provider request failed: {0}")]
    Provider(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    NangoConnection(#[from] anlg_api_nango::NangoConnectionError),
}

impl IntoResponse for CrmError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Provider(message) => (StatusCode::BAD_GATEWAY, "provider_error", message),
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
