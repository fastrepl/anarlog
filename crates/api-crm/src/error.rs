use anlg_api_nango::{NangoConnectionError, NangoConnectionState, is_provider_auth_failure};
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

    #[error("Contact not found: {0}")]
    NotFound(String),

    #[error("CRM provider request failed: {0}")]
    Provider(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    NangoConnection(#[from] anlg_api_nango::NangoConnectionError),
}

pub async fn map_provider_error(
    nango_state: &NangoConnectionState,
    integration_id: &str,
    connection_id: &str,
    err: CrmError,
) -> CrmError {
    let CrmError::Provider(ref message) = err else {
        return err;
    };
    if !is_provider_auth_failure(message) {
        return err;
    }

    if let Err(mark_err) = nango_state
        .mark_reconnect_required(integration_id, connection_id, message)
        .await
    {
        tracing::warn!(
            error = %mark_err,
            integration_id,
            connection_id,
            "failed to persist crm reconnect_required"
        );
    }

    CrmError::NangoConnection(NangoConnectionError::ReconnectRequired(
        integration_id.to_string(),
    ))
}

impl IntoResponse for CrmError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "contact_not_found", message),
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn map_provider_error_marks_reconnect_required_on_auth_failure() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/rest/v1/nango_connections"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let config = anlg_api_nango::NangoConfig::new(
            &anlg_api_env::NangoEnv {
                nango_api_base: Some(server.uri()),
                nango_api_key: "fixture-key".to_string(),
                nango_webhook_signing_key: "fixture-signing".to_string(),
            },
            &anlg_api_env::SupabaseEnv {
                supabase_url: server.uri(),
                supabase_anon_key: "fixture-anon".to_string(),
                supabase_service_role_key: "service-key".to_string(),
            },
            Some("service-key".to_string()),
        );
        let state = NangoConnectionState::from_config(&config);
        let err = map_provider_error(
            &state,
            "hubspot",
            "conn-1",
            CrmError::Provider("API error (status 401): invalid_grant".to_string()),
        )
        .await;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FAILED_DEPENDENCY);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"]["code"], "reconnect_required");

        let transient = map_provider_error(
            &state,
            "hubspot",
            "conn-1",
            CrmError::Provider("API error (status 503): unavailable".to_string()),
        )
        .await;
        assert_eq!(transient.into_response().status(), StatusCode::BAD_GATEWAY);
    }
}
