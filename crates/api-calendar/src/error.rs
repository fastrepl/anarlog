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
    if integration_id == "outlook" && message.contains("MailboxNotEnabledForRESTAPI") {
        return CalendarError::MailboxUnavailable;
    }
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

    #[error("Outlook mailbox is unavailable for calendar access")]
    MailboxUnavailable,

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
            Self::MailboxUnavailable => (
                StatusCode::FAILED_DEPENDENCY,
                "outlook_mailbox_unavailable",
                "This Microsoft account does not have an active cloud mailbox. Ask your Microsoft 365 administrator to enable Exchange Online, or connect a different Outlook account.".to_string(),
            ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unavailable_outlook_mailbox_is_actionable_without_requesting_reauthorization() {
        let nango = anlg_nango::NangoClient::builder()
            .api_key("fixture-key")
            .api_base("http://127.0.0.1:1")
            .build()
            .unwrap();
        let state = NangoConnectionState::new(nango, "http://127.0.0.1:1", "fixture-key");
        let provider_error = r#"HTTP client error: API error (404 Not Found): {"error":{"code":"MailboxNotEnabledForRESTAPI","message":"The mailbox is either inactive, soft-deleted, or is hosted on-premise."}}"#;
        let error = map_provider_error(&state, "outlook", "fixture", provider_error).await;
        assert!(matches!(error, CalendarError::MailboxUnavailable));
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::FAILED_DEPENDENCY);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"]["code"], "outlook_mailbox_unavailable");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Exchange Online")
        );
        for (integration, message) in [
            ("google-calendar", provider_error),
            ("outlook", "API error (503 Service Unavailable)"),
            ("outlook", "API error (404 Not Found): ErrorItemNotFound"),
        ] {
            assert_eq!(
                map_provider_error(&state, integration, "fixture", message)
                    .await
                    .into_response()
                    .status(),
                StatusCode::INTERNAL_SERVER_ERROR
            );
        }
    }
}
