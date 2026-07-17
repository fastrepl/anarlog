use hypr_supabase_storage::SupabaseStorage;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("{0}")]
    Api(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PipelineStatus {
    Processing,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionJob {
    pub id: String,
    pub user_id: String,
    pub file_id: String,
    pub provider: String,
    pub status: PipelineStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobUpdate {
    pub status: PipelineStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct SupabaseClient {
    client: reqwest::Client,
    base_url: String,
    service_role_key: String,
}

impl SupabaseClient {
    pub fn new(client: reqwest::Client, supabase_url: &str, service_role_key: &str) -> Self {
        Self {
            client,
            base_url: supabase_url.trim_end_matches('/').to_string(),
            service_role_key: service_role_key.to_string(),
        }
    }

    pub fn storage(&self) -> SupabaseStorage {
        SupabaseStorage::new(self.client.clone(), &self.base_url, &self.service_role_key)
    }

    fn rest_url(&self) -> String {
        format!("{}/rest/v1/transcription_jobs", self.base_url)
    }

    fn auth_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        hypr_observability::with_current_trace_context(
            builder
                .header("Authorization", format!("Bearer {}", self.service_role_key))
                .header("apikey", &self.service_role_key),
        )
    }

    pub async fn insert_job(&self, job: &TranscriptionJob) -> Result<(), Error> {
        let start = Instant::now();
        let response = self
            .auth_headers(self.client.post(self.rest_url()))
            .header("Prefer", "return=minimal")
            .json(job)
            .send()
            .await?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "insert_job",
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(
                service.peer.name = "supabase",
                hyprnote.supabase.operation = "insert_job",
                http.response.status_code = status.as_u16(),
                error.type = "supabase_api_error",
                "supabase_request_failed"
            );
            return Err(Error::Api(format!("failed to insert job: {status} {body}")));
        }

        Ok(())
    }

    pub async fn update_job(&self, id: &str, updates: &JobUpdate) -> Result<(), Error> {
        let encoded_id = urlencoding::encode(id);
        let url = format!("{}?id=eq.{encoded_id}", self.rest_url());

        let start = Instant::now();
        let response = self
            .auth_headers(self.client.patch(&url))
            .header("Prefer", "return=minimal")
            .json(updates)
            .send()
            .await?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "update_job",
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(
                service.peer.name = "supabase",
                hyprnote.supabase.operation = "update_job",
                http.response.status_code = status.as_u16(),
                error.type = "supabase_api_error",
                "supabase_request_failed"
            );
            return Err(Error::Api(format!("failed to update job: {status} {body}")));
        }

        Ok(())
    }

    pub async fn get_job(&self, id: &str) -> Result<Option<TranscriptionJob>, Error> {
        let encoded_id = urlencoding::encode(id);
        let url = format!("{}?id=eq.{encoded_id}&select=*", self.rest_url());

        let start = Instant::now();
        let response = self
            .auth_headers(self.client.get(&url))
            .header("Accept", "application/json")
            .send()
            .await?;
        tracing::info!(
            service.peer.name = "supabase",
            hyprnote.supabase.operation = "get_job",
            http.response.status_code = response.status().as_u16(),
            hyprnote.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(
                service.peer.name = "supabase",
                hyprnote.supabase.operation = "get_job",
                http.response.status_code = status.as_u16(),
                error.type = "supabase_api_error",
                "supabase_request_failed"
            );
            return Err(Error::Api(format!("failed to get job: {status} {body}")));
        }

        let jobs: Vec<TranscriptionJob> = response.json().await?;
        Ok(jobs.into_iter().next())
    }
}
