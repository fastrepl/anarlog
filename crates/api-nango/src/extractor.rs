use std::marker::PhantomData;

use anlg_api_auth::AuthContext;
use anlg_nango::{NangoClient, OwnedNangoHttpClient, OwnedNangoProxy};
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};

use crate::integrations::NangoIntegrationId;
use crate::supabase::SupabaseClient;

#[derive(Clone)]
pub struct NangoConnectionState {
    nango: NangoClient,
    http_client: reqwest::Client,
    supabase_url: String,
    supabase_anon_key: String,
    supabase: SupabaseClient,
}

impl NangoConnectionState {
    pub fn new(
        nango: NangoClient,
        supabase_url: impl Into<String>,
        supabase_anon_key: impl Into<String>,
    ) -> Self {
        Self::with_service_role(nango, supabase_url, supabase_anon_key, None)
    }

    pub fn from_config(config: &crate::config::NangoConfig) -> Self {
        let nango = crate::config::build_nango_client(config).expect("failed to build NangoClient");

        Self::with_service_role(
            nango,
            &config.supabase_url,
            &config.supabase_anon_key,
            config.supabase_service_role_key.clone(),
        )
    }

    fn with_service_role(
        nango: NangoClient,
        supabase_url: impl Into<String>,
        supabase_anon_key: impl Into<String>,
        supabase_service_role_key: Option<String>,
    ) -> Self {
        let supabase_url = supabase_url.into();
        let supabase_anon_key = supabase_anon_key.into();
        Self {
            nango,
            http_client: reqwest::Client::new(),
            supabase_url: supabase_url.trim_end_matches('/').to_string(),
            supabase_anon_key: supabase_anon_key.clone(),
            supabase: SupabaseClient::new(
                supabase_url,
                supabase_anon_key,
                supabase_service_role_key,
            ),
        }
    }

    pub async fn mark_reconnect_required(
        &self,
        integration_id: &str,
        connection_id: &str,
        error_description: &str,
    ) -> Result<(), NangoConnectionError> {
        self.supabase
            .mark_connection_refresh_failed(
                integration_id,
                connection_id,
                Some("provider_auth"),
                Some(error_description),
            )
            .await
            .map_err(|e| NangoConnectionError::Database(e.to_string()))
    }

    pub async fn build_http_client(
        &self,
        auth_token: &str,
        user_id: &str,
        integration_id: &str,
        connection_id: &str,
    ) -> Result<OwnedNangoHttpClient, NangoConnectionError> {
        let encoded_user_id = urlencoding::encode(user_id);
        let encoded_connection_id = urlencoding::encode(connection_id);
        let encoded_integration_id = urlencoding::encode(integration_id);
        let url = format!(
            "{}/rest/v1/nango_connections?select=connection_id,status&user_id=eq.{}&connection_id=eq.{}&integration_id=eq.{}",
            self.supabase_url, encoded_user_id, encoded_connection_id, encoded_integration_id,
        );

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .header("apikey", &self.supabase_anon_key)
            .send()
            .await
            .map_err(|e| NangoConnectionError::Database(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(NangoConnectionError::Database(format!(
                "query failed: {} - {}",
                status, body
            )));
        }

        #[derive(serde::Deserialize)]
        struct Row {
            #[serde(default)]
            status: String,
        }

        let rows: Vec<Row> = response
            .json()
            .await
            .map_err(|e| NangoConnectionError::Database(e.to_string()))?;

        match rows.into_iter().next() {
            Some(row) if row.status == "reconnect_required" => {
                return Err(NangoConnectionError::ReconnectRequired(
                    integration_id.to_string(),
                ));
            }
            Some(_) => {}
            None => {
                return Err(NangoConnectionError::NotConnected(
                    integration_id.to_string(),
                ));
            }
        }

        let proxy = OwnedNangoProxy::new(
            &self.nango,
            integration_id.to_string(),
            connection_id.to_string(),
        );
        Ok(OwnedNangoHttpClient::new(proxy))
    }

    async fn get_connection_id(
        &self,
        auth_token: &str,
        user_id: &str,
        integration_id: &str,
    ) -> Result<String, NangoConnectionError> {
        #[cfg(debug_assertions)]
        if let Ok(connection_id) = std::env::var("DEV_NANGO_CONNECTION_ID")
            && !connection_id.is_empty()
        {
            return Ok(connection_id);
        }

        let encoded_user_id = urlencoding::encode(user_id);
        let encoded_integration_id = urlencoding::encode(integration_id);
        let url = format!(
            "{}/rest/v1/nango_connections?select=connection_id,status&user_id=eq.{}&integration_id=eq.{}",
            self.supabase_url, encoded_user_id, encoded_integration_id,
        );

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .header("apikey", &self.supabase_anon_key)
            .send()
            .await
            .map_err(|e| NangoConnectionError::Database(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(NangoConnectionError::Database(format!(
                "query failed: {} - {}",
                status, body
            )));
        }

        let rows: Vec<crate::supabase::NangoConnectionRow> = response
            .json()
            .await
            .map_err(|e| NangoConnectionError::Database(e.to_string()))?;

        match rows.into_iter().next() {
            Some(row) if row.status == "reconnect_required" => Err(
                NangoConnectionError::ReconnectRequired(integration_id.to_string()),
            ),
            Some(row) => Ok(row.connection_id),
            None => Err(NangoConnectionError::NotConnected(
                integration_id.to_string(),
            )),
        }
    }
}

pub struct NangoConnection<I: NangoIntegrationId> {
    http: OwnedNangoHttpClient,
    _marker: PhantomData<I>,
}

impl<I: NangoIntegrationId> NangoConnection<I> {
    pub fn into_http(self) -> OwnedNangoHttpClient {
        self.http
    }
}

#[derive(Debug)]
pub enum NangoConnectionError {
    NotAuthenticated,
    NotConnected(String),
    ReconnectRequired(String),
    MissingState,
    Database(String),
}

impl IntoResponse for NangoConnectionError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::NotAuthenticated => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "not authenticated".to_string(),
            ),
            Self::NotConnected(integration_id) => (
                StatusCode::BAD_REQUEST,
                "not_connected",
                format!("no connection found for integration: {}", integration_id),
            ),
            Self::ReconnectRequired(integration_id) => (
                StatusCode::FAILED_DEPENDENCY,
                "reconnect_required",
                format!(
                    "connection requires reconnect for integration: {}",
                    integration_id
                ),
            ),
            Self::MissingState => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                "NangoConnectionState not found in request extensions".to_string(),
            ),
            Self::Database(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                msg.clone(),
            ),
        };

        anlg_api_error::error_response(status, code, &message)
    }
}

impl std::fmt::Display for NangoConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAuthenticated => write!(f, "not authenticated"),
            Self::NotConnected(id) => write!(f, "not connected: {}", id),
            Self::ReconnectRequired(id) => write!(f, "reconnect required: {}", id),
            Self::MissingState => write!(f, "missing NangoConnectionState"),
            Self::Database(msg) => write!(f, "database error: {}", msg),
        }
    }
}

impl std::error::Error for NangoConnectionError {}

pub fn is_provider_auth_failure(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("(401")
        || lower.contains("status 401")
        || lower.contains("invalidauthenticationtoken")
        || lower.contains("invalid_credentials")
        || lower.contains("token is expired")
        || lower.contains("lifetime validation failed")
        || lower.contains("invalid_grant")
        || lower.contains("could not refresh")
        || lower.contains("refresh access token")
}

impl<S: Send + Sync, I: NangoIntegrationId> FromRequestParts<S> for NangoConnection<I> {
    type Rejection = NangoConnectionError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth = parts
            .extensions
            .get::<AuthContext>()
            .ok_or(NangoConnectionError::NotAuthenticated)?;

        let nango_state = parts
            .extensions
            .get::<NangoConnectionState>()
            .ok_or(NangoConnectionError::MissingState)?;

        let connection_id = nango_state
            .get_connection_id(&auth.token, &auth.claims.sub, I::ID)
            .await?;

        let proxy = OwnedNangoProxy::new(&nango_state.nango, I::ID.to_string(), connection_id);
        let http = OwnedNangoHttpClient::new(proxy);

        Ok(NangoConnection {
            http,
            _marker: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NangoConfig;
    use wiremock::matchers::{method, path_regex, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn detects_graph_expired_token() {
        assert!(is_provider_auth_failure(
            "HTTP status client error (401 Unauthorized) for url (https://api.nango.dev/proxy/me/calendars)"
        ));
        assert!(is_provider_auth_failure(
            r#"{"error":{"code":"InvalidAuthenticationToken","message":"Lifetime validation failed, the token is expired."}}"#
        ));
        assert!(is_provider_auth_failure("invalid_grant"));
        assert!(is_provider_auth_failure(
            r#"API error (status 500): {"error":{"code":"InvalidAuthenticationToken","message":"Lifetime validation failed, the token is expired."}}"#
        ));
        assert!(is_provider_auth_failure(
            r#"API error (status 500): {"error":{"code":"invalid_credentials","message":"Could not refresh access token for connection"}}"#
        ));
        assert!(!is_provider_auth_failure(
            "HTTP status server error (500 Internal Server Error) for url (https://api.nango.dev/proxy/me/calendars)"
        ));
        assert!(!is_provider_auth_failure(
            "API error (status 500): Internal Server Error"
        ));
        assert!(!is_provider_auth_failure(
            "HTTP status client error (403 Forbidden) for url (https://api.nango.dev/proxy/me/calendars/AAMk)"
        ));
    }

    #[tokio::test]
    async fn mark_reconnect_required_patches_connection() {
        let nango_mock = MockServer::start().await;
        let supabase_mock = MockServer::start().await;
        let config = NangoConfig::for_test(&nango_mock.uri(), &supabase_mock.uri());
        let state = NangoConnectionState::from_config(&config);

        Mock::given(method("PATCH"))
            .and(path_regex("/rest/v1/nango_connections"))
            .and(query_param("integration_id", "eq.outlook"))
            .and(query_param("connection_id", "eq.conn-123"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&supabase_mock)
            .await;

        state
            .mark_reconnect_required("outlook", "conn-123", "token is expired")
            .await
            .unwrap();

        supabase_mock.verify().await;
    }
}
