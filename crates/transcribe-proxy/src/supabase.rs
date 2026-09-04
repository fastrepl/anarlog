use anlg_supabase_storage::SupabaseStorage;
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
    pub provider_request_id: Option<String>,
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
        anlg_observability::with_current_trace_context(
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
            anarlog.supabase.operation = "insert_job",
            http.response.status_code = response.status().as_u16(),
            anarlog.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            tracing::error!(
                service.peer.name = "supabase",
                anarlog.supabase.operation = "insert_job",
                http.response.status_code = status.as_u16(),
                error.type = "supabase_api_error",
                "supabase_request_failed"
            );
            return Err(Error::Api(format!("failed to insert job: {status}")));
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
            anarlog.supabase.operation = "update_job",
            http.response.status_code = response.status().as_u16(),
            anarlog.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            tracing::error!(
                service.peer.name = "supabase",
                anarlog.supabase.operation = "update_job",
                http.response.status_code = status.as_u16(),
                error.type = "supabase_api_error",
                "supabase_request_failed"
            );
            return Err(Error::Api(format!("failed to update job: {status}")));
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
            anarlog.supabase.operation = "get_job",
            http.response.status_code = response.status().as_u16(),
            anarlog.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            tracing::error!(
                service.peer.name = "supabase",
                anarlog.supabase.operation = "get_job",
                http.response.status_code = status.as_u16(),
                error.type = "supabase_api_error",
                "supabase_request_failed"
            );
            return Err(Error::Api(format!("failed to get job: {status}")));
        }

        let jobs: Vec<TranscriptionJob> = response.json().await?;
        Ok(jobs.into_iter().next())
    }

    pub async fn get_job_for_user(
        &self,
        id: &str,
        user_id: &str,
    ) -> Result<Option<TranscriptionJob>, Error> {
        let encoded_id = urlencoding::encode(id);
        let encoded_user_id = urlencoding::encode(user_id);
        let url = format!(
            "{}?id=eq.{encoded_id}&user_id=eq.{encoded_user_id}&select=*",
            self.rest_url()
        );

        let start = Instant::now();
        let response = self
            .auth_headers(self.client.get(&url))
            .header("Accept", "application/json")
            .send()
            .await?;
        tracing::info!(
            service.peer.name = "supabase",
            anarlog.supabase.operation = "get_job_for_user",
            http.response.status_code = response.status().as_u16(),
            anarlog.duration_ms = start.elapsed().as_millis() as u64,
            "supabase_request_finished"
        );

        if !response.status().is_success() {
            let status = response.status();
            tracing::error!(
                service.peer.name = "supabase",
                anarlog.supabase.operation = "get_job_for_user",
                http.response.status_code = status.as_u16(),
                error.type = "supabase_api_error",
                "supabase_request_failed"
            );
            return Err(Error::Api(format!("failed to get job: {status}")));
        }

        let jobs: Vec<TranscriptionJob> = response.json().await?;
        Ok(jobs.into_iter().next())
    }
}

pub(crate) fn user_owns_object_path(user_id: &str, object_path: &str) -> bool {
    let mut segments = object_path.split('/');
    matches!(segments.next(), Some(owner) if owner == user_id)
        && segments.next().is_some_and(|segment| !segment.is_empty())
}

#[cfg(test)]
mod tests {
    use super::user_owns_object_path;

    #[test]
    fn accepts_only_objects_nested_under_the_exact_user_id() {
        let user_id = "00000000-0000-4000-8000-000000000001";

        assert!(user_owns_object_path(
            user_id,
            &format!("{user_id}/audio.m4a")
        ));
        assert!(!user_owns_object_path(user_id, "audio.m4a"));
        assert!(!user_owns_object_path(user_id, &format!("{user_id}/")));
        assert!(!user_owns_object_path(
            user_id,
            "00000000-0000-4000-8000-000000000002/audio.m4a"
        ));
        assert!(!user_owns_object_path(
            user_id,
            &format!("{user_id}%2Faudio.m4a")
        ));
    }
}
