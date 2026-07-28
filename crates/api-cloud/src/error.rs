use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, thiserror::Error)]
pub enum CloudApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("subscription required")]
    SubscriptionRequired,
    #[error("cloud API not enabled")]
    NotEnabled,
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Internal(String),
}

#[derive(Serialize, ToSchema)]
pub struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize, ToSchema)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl IntoResponse for CloudApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Provide a valid cloud API key as `Authorization: Bearer <key>`.".to_string(),
            ),
            Self::SubscriptionRequired => (
                StatusCode::FORBIDDEN,
                "subscription_required",
                "An active Anarlog Pro subscription is required.".to_string(),
            ),
            Self::NotEnabled => (
                StatusCode::FORBIDDEN,
                "cloud_api_not_enabled",
                "Enable Cloud API & Connectors in Anarlog before using this endpoint.".to_string(),
            ),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
            Self::InvalidRequest(message) => (StatusCode::BAD_REQUEST, "invalid_request", message),
            Self::Internal(message) => {
                tracing::error!("[cloud-api] internal error: {message}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Something went wrong while handling the request.".to_string(),
                )
            }
        };

        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}
