use std::marker::PhantomData;

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use hypr_api_auth::AuthContext;
use hypr_nango::{NangoClient, OwnedNangoHttpClient, OwnedNangoProxy};

use crate::integrations::NangoIntegrationId;

#[derive(Clone)]
pub struct NangoConnectionState {
    nango: NangoClient,
    http_client: reqwest::Client,
    supabase_url: String,
    supabase_anon_key: String,
}

impl NangoConnectionState {
    pub fn new(
        nango: NangoClient,
        supabase_url: impl Into<String>,
        supabase_anon_key: impl Into<String>,
    ) -> Self {
        Self {
            nango,
            http_client: reqwest::Client::new(),
            supabase_url: supabase_url.into().trim_end_matches('/').to_string(),
            supabase_anon_key: supabase_anon_key.into(),
        }
    }

    pub fn from_config(config: &crate::config::NangoConfig) -> Self {
        let nango = crate::config::build_nango_client(config).expect("failed to build NangoClient");

        Self::new(nango, &config.supabase_url, &config.supabase_anon_key)
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

        hypr_api_error::error_response(status, code, &message)
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
