use anlg_agent_access::MeetingExport;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::routes::{ApiKeyInfo, CloudApiSettings, SnapshotReceipt};

const MAX_RPC_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_RPC_ERROR_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct CloudApiConfig {
    supabase_url: String,
    supabase_service_role_key: String,
}

impl CloudApiConfig {
    pub fn new(
        supabase_url: impl Into<String>,
        supabase_service_role_key: impl Into<String>,
    ) -> Result<Self, String> {
        let supabase_url = supabase_url.into().trim_end_matches('/').to_string();
        let supabase_service_role_key = supabase_service_role_key.into();
        if !supabase_url.starts_with("https://") && !supabase_url.starts_with("http://") {
            return Err("Supabase URL must use http or https".to_string());
        }
        if supabase_service_role_key.trim().is_empty() {
            return Err("Supabase service role key is required".to_string());
        }
        Ok(Self {
            supabase_url,
            supabase_service_role_key,
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    client: reqwest::Client,
    config: CloudApiConfig,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StoreError {
    #[error("cloud data request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("cloud data request returned {status}: {body}")]
    Response { status: StatusCode, body: String },
    #[error("cloud data response was invalid: {0}")]
    InvalidResponse(#[from] serde_json::Error),
    #[error("cloud data response exceeded the {limit}-byte limit")]
    ResponseTooLarge { limit: usize },
}

#[derive(Deserialize)]
struct SnapshotRow {
    content_json: MeetingExport,
}

#[derive(Deserialize)]
pub(crate) struct VerifiedKey {
    pub user_id: String,
    pub status: String,
}

impl AppState {
    pub fn new(config: CloudApiConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    async fn rpc<T: DeserializeOwned>(
        &self,
        function: &str,
        body: impl Serialize,
    ) -> Result<T, StoreError> {
        let response = self
            .client
            .post(format!(
                "{}/rest/v1/rpc/{function}",
                self.config.supabase_url
            ))
            .header("apikey", &self.config.supabase_service_role_key)
            .bearer_auth(&self.config.supabase_service_role_key)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let response_limit = if status.is_success() {
            MAX_RPC_RESPONSE_BYTES
        } else {
            MAX_RPC_ERROR_BYTES
        };
        let bytes = read_response_body(response, response_limit).await?;
        if !status.is_success() {
            return Err(StoreError::Response {
                status,
                body: String::from_utf8_lossy(&bytes).into_owned(),
            });
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub(crate) async fn get_settings(&self, user_id: &str) -> Result<CloudApiSettings, StoreError> {
        let mut rows: Vec<CloudApiSettings> = self
            .rpc(
                "get_cloud_api_settings",
                serde_json::json!({ "p_actor_user_id": user_id }),
            )
            .await?;
        rows.pop().ok_or_else(|| {
            StoreError::InvalidResponse(serde_json::Error::io(std::io::Error::other(
                "settings row missing",
            )))
        })
    }

    pub(crate) async fn set_enabled(
        &self,
        user_id: &str,
        enabled: bool,
    ) -> Result<CloudApiSettings, StoreError> {
        let mut rows: Vec<CloudApiSettings> = self
            .rpc(
                "set_cloud_api_enabled",
                serde_json::json!({
                    "p_actor_user_id": user_id,
                    "p_enabled": enabled,
                }),
            )
            .await?;
        rows.pop().ok_or_else(|| {
            StoreError::InvalidResponse(serde_json::Error::io(std::io::Error::other(
                "settings row missing",
            )))
        })
    }

    pub(crate) async fn publish_snapshot(
        &self,
        user_id: &str,
        session_id: &str,
        content: &MeetingExport,
    ) -> Result<SnapshotReceipt, StoreError> {
        let mut rows: Vec<SnapshotReceipt> = self
            .rpc(
                "publish_cloud_api_snapshot",
                serde_json::json!({
                    "p_actor_user_id": user_id,
                    "p_session_id": session_id,
                    "p_content_json": content,
                }),
            )
            .await?;
        rows.pop().ok_or_else(|| {
            StoreError::InvalidResponse(serde_json::Error::io(std::io::Error::other(
                "snapshot receipt missing",
            )))
        })
    }

    pub(crate) async fn delete_snapshot(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> Result<bool, StoreError> {
        self.rpc(
            "delete_cloud_api_snapshot",
            serde_json::json!({
                "p_actor_user_id": user_id,
                "p_session_id": session_id,
            }),
        )
        .await
    }

    pub(crate) async fn list_snapshots(
        &self,
        user_id: &str,
        query: Option<&str>,
        series_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MeetingExport>, StoreError> {
        let rows: Vec<SnapshotRow> = self
            .rpc(
                "list_cloud_api_snapshots",
                serde_json::json!({
                    "p_user_id": user_id,
                    "p_query": query,
                    "p_series_id": series_id,
                    "p_limit": limit,
                    "p_offset": offset,
                }),
            )
            .await?;
        Ok(rows.into_iter().map(|row| row.content_json).collect())
    }

    pub(crate) async fn read_snapshot(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> Result<Option<MeetingExport>, StoreError> {
        let mut rows: Vec<SnapshotRow> = self
            .rpc(
                "read_cloud_api_snapshot",
                serde_json::json!({
                    "p_user_id": user_id,
                    "p_session_id": session_id,
                }),
            )
            .await?;
        Ok(rows.pop().map(|row| row.content_json))
    }

    pub(crate) async fn create_key(
        &self,
        user_id: &str,
        id: &str,
        name: &str,
        key_hash: &str,
        key_prefix: &str,
    ) -> Result<ApiKeyInfo, StoreError> {
        let mut rows: Vec<ApiKeyInfo> = self
            .rpc(
                "create_cloud_api_key",
                serde_json::json!({
                    "p_actor_user_id": user_id,
                    "p_id": id,
                    "p_name": name,
                    "p_key_hash": key_hash,
                    "p_key_prefix": key_prefix,
                }),
            )
            .await?;
        rows.pop().ok_or_else(|| {
            StoreError::InvalidResponse(serde_json::Error::io(std::io::Error::other(
                "API key row missing",
            )))
        })
    }

    pub(crate) async fn list_keys(&self, user_id: &str) -> Result<Vec<ApiKeyInfo>, StoreError> {
        self.rpc(
            "list_cloud_api_keys",
            serde_json::json!({ "p_actor_user_id": user_id }),
        )
        .await
    }

    pub(crate) async fn revoke_key(&self, user_id: &str, key_id: &str) -> Result<bool, StoreError> {
        self.rpc(
            "revoke_cloud_api_key",
            serde_json::json!({
                "p_actor_user_id": user_id,
                "p_key_id": key_id,
            }),
        )
        .await
    }

    pub(crate) async fn verify_key(
        &self,
        key_hash: &str,
    ) -> Result<Option<VerifiedKey>, StoreError> {
        let mut rows: Vec<VerifiedKey> = self
            .rpc(
                "verify_cloud_api_key",
                serde_json::json!({ "p_key_hash": key_hash }),
            )
            .await?;
        Ok(rows.pop())
    }
}

async fn read_response_body(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, StoreError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(StoreError::ResponseTooLarge { limit });
    }

    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or_default()
            .min(limit),
    );
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(StoreError::ResponseTooLarge { limit });
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

    use super::{StoreError, read_response_body};

    #[tokio::test]
    async fn rejects_response_bodies_over_the_limit() {
        let server = MockServer::start().await;
        Mock::given(path("/large"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 17]))
            .mount(&server)
            .await;

        let response = reqwest::get(format!("{}/large", server.uri()))
            .await
            .unwrap();
        let error = read_response_body(response, 16)
            .await
            .expect_err("oversized response must fail");
        assert!(matches!(error, StoreError::ResponseTooLarge { limit: 16 }));
    }
}
