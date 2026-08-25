use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, thiserror::Error)]
pub enum CloudApiError {
    #[error("unauthorized")]
    Unauthorized(String),
    #[error("insufficient OAuth scope")]
    InsufficientScope(String),
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
        let (status, code, message, challenge) = match self {
            Self::Unauthorized(challenge) => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Provide a valid cloud API key or OAuth access token as `Authorization: Bearer <token>`.".to_string(),
                Some(challenge),
            ),
            Self::InsufficientScope(challenge) => (
                StatusCode::FORBIDDEN,
                "insufficient_scope",
                "The OAuth access token does not grant the required scopes.".to_string(),
                Some(challenge),
            ),
            Self::SubscriptionRequired => (
                StatusCode::FORBIDDEN,
                "subscription_required",
                "An active Anarlog Pro subscription is required.".to_string(),
                None,
            ),
            Self::NotEnabled => (
                StatusCode::FORBIDDEN,
                "cloud_api_not_enabled",
                "Enable Cloud API & Connectors in Anarlog before using this endpoint.".to_string(),
                None,
            ),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message, None),
            Self::InvalidRequest(message) => {
                (StatusCode::BAD_REQUEST, "invalid_request", message, None)
            }
            Self::Internal(message) => {
                tracing::error!("[cloud-api] internal error: {message}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Something went wrong while handling the request.".to_string(),
                    None,
                )
            }
        };

        let mut response = (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
            }),
        )
            .into_response();
        if let Some(challenge) = challenge {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_str(&challenge).expect("OAuth challenge must be a valid header"),
            );
        }
        response
    }
}
