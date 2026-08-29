use anlg_api_nango::{NangoConnectionError, NangoConnectionState, is_provider_auth_failure};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CalendarError>;

pub async fn map_provider_error(
    nango_state: &NangoConnectionState,
    integration_id: &str,
    connection_id: &str,
    err: impl std::fmt::Display,
) -> CalendarError {
    let message = err.to_string();
    if !is_provider_auth_failure(&message) {
        return CalendarError::Internal(message);
    }

    if let Err(mark_err) = nango_state
        .mark_reconnect_required(integration_id, connection_id, &message)
        .await
    {
        tracing::warn!(
            error = %mark_err,
            integration_id,
            connection_id,
            "failed to persist calendar reconnect_required"
        );
    }

    CalendarError::NangoConnection(NangoConnectionError::ReconnectRequired(
        integration_id.to_string(),
    ))
}

#[derive(Debug, Error)]
pub enum CalendarError {
    #[error("Authentication error: {0}")]
    #[allow(dead_code)]
    Auth(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    NangoConnection(#[from] anlg_api_nango::NangoConnectionError),
}

impl IntoResponse for CalendarError {
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

        anlg_api_error::error_response(status, code, &message)
    }
}
