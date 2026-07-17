use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SubscriptionError>;

#[derive(Debug, Error)]
pub enum SubscriptionError {
    #[error("Supabase request failed: {0}")]
    SupabaseRequest(String),

    #[error("Stripe error: {0}")]
    Stripe(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<stripe::StripeError> for SubscriptionError {
    fn from(err: stripe::StripeError) -> Self {
        Self::Stripe(err.to_string())
    }
}

impl IntoResponse for SubscriptionError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::SupabaseRequest(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "supabase_error", msg)
            }
            Self::Stripe(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "stripe_error", msg),
            Self::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                msg,
            ),
        };

        hypr_api_error::error_response(status, code, &message)
    }
}
