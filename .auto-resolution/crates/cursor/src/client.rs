use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue};

use crate::{
    Agent, AgentConversation, AgentIdResponse, ApiKeyInfo, DownloadArtifactRequest,
    DownloadArtifactResponse, Error, FollowupRequest, LaunchAgentRequest, ListAgentsRequest,
    ListAgentsResponse, ListArtifactsResponse, ListModelsResponse, ListRepositoriesResponse,
};

#[derive(Default)]
pub struct CursorClientBuilder {
    api_key: Option<String>,
    api_base: Option<String>,
    client: Option<reqwest::Client>,
}

impl CursorClientBuilder {
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.client = Some(client);
        self
    }

    pub fn build(self) -> Result<CursorClient, Error> {
        let api_key = self.api_key.ok_or(Error::MissingApiKey)?;
        let encoded = STANDARD.encode(format!("{api_key}:"));
        let auth_header =
            HeaderValue::from_str(&format!("Basic {encoded}")).map_err(|_| Error::InvalidApiKey)?;

        let client = self.client.unwrap_or_else(reqwest::Client::new);

        let mut api_base: url::Url = self
            .api_base
            .unwrap_or_else(|| "https://api.cursor.com".to_string())
            .parse()?;
        if !api_base.path().ends_with('/') {
            let path = format!("{}/", api_base.path());
            api_base.set_path(&path);
        }

        Ok(CursorClient {
            client,
            api_base,
            auth_header,
        })
    }
}

#[derive(Clone)]
pub struct CursorClient {
    client: reqwest::Client,
    api_base: url::Url,
    auth_header: HeaderValue,
}

impl CursorClient {
    pub fn builder() -> CursorClientBuilder {
        CursorClientBuilder::default()
    }

    pub fn api_base(&self) -> &url::Url {
        &self.api_base
    }

    pub async fn list_agents(&self, req: ListAgentsRequest) -> Result<ListAgentsResponse, Error> {
        let mut url = self.endpoint("/v0/agents")?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(limit) = req.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(cursor) = req.cursor.as_deref() {
                pairs.append_pair("cursor", cursor);
            }
            if let Some(pr_url) = req.pr_url.as_deref() {
                pairs.append_pair("prUrl", pr_url);
            }
        }

        self.send(self.client.get(url)).await
    }

    pub async fn get_agent(&self, id: &str) -> Result<Agent, Error> {
        let url = self.endpoint(&format!("/v0/agents/{id}"))?;
        self.send(self.client.get(url)).await
    }

    pub async fn get_agent_conversation(&self, id: &str) -> Result<AgentConversation, Error> {
        let url = self.endpoint(&format!("/v0/agents/{id}/conversation"))?;
        self.send(self.client.get(url)).await
    }

    pub async fn list_artifacts(&self, id: &str) -> Result<ListArtifactsResponse, Error> {
        let url = self.endpoint(&format!("/v0/agents/{id}/artifacts"))?;
        self.send(self.client.get(url)).await
    }

    pub async fn download_artifact(
        &self,
        id: &str,
        req: DownloadArtifactRequest,
    ) -> Result<DownloadArtifactResponse, Error> {
        let mut url = self.endpoint(&format!("/v0/agents/{id}/artifacts/download"))?;
        url.query_pairs_mut().append_pair("path", &req.path);

        self.send(self.client.get(url)).await
    }

    pub async fn launch_agent(&self, req: &LaunchAgentRequest) -> Result<Agent, Error> {
        let url = self.endpoint("/v0/agents")?;
        self.send(self.client.post(url).json(req)).await
    }

    pub async fn add_followup(
        &self,
        id: &str,
        req: &FollowupRequest,
    ) -> Result<AgentIdResponse, Error> {
        let url = self.endpoint(&format!("/v0/agents/{id}/followup"))?;
        self.send(self.client.post(url).json(req)).await
    }

    pub async fn stop_agent(&self, id: &str) -> Result<AgentIdResponse, Error> {
        let url = self.endpoint(&format!("/v0/agents/{id}/stop"))?;
        self.send(self.client.post(url)).await
    }

    pub async fn delete_agent(&self, id: &str) -> Result<AgentIdResponse, Error> {
        let url = self.endpoint(&format!("/v0/agents/{id}"))?;
        self.send(self.client.delete(url)).await
    }

    pub async fn me(&self) -> Result<ApiKeyInfo, Error> {
        let url = self.endpoint("/v0/me")?;
        self.send(self.client.get(url)).await
    }

    pub async fn list_models(&self) -> Result<ListModelsResponse, Error> {
        let url = self.endpoint("/v0/models")?;
        self.send(self.client.get(url)).await
    }

    pub async fn list_repositories(&self) -> Result<ListRepositoriesResponse, Error> {
        let url = self.endpoint("/v0/repositories")?;
        self.send(self.client.get(url)).await
    }

    fn endpoint(&self, path: &str) -> Result<url::Url, Error> {
        self.api_base
            .join(path.trim_start_matches('/'))
            .map_err(Error::from)
    }

    async fn send<T>(&self, request: reqwest::RequestBuilder) -> Result<T, Error>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut auth_header = self.auth_header.clone();
        auth_header.set_sensitive(true);

        let response = request
            .header(AUTHORIZATION, auth_header)
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .send()
            .await?;
        let status = response.status();

        if status.is_success() {
            return Ok(response.json().await?);
        }

        let message = response.text().await.unwrap_or_default();
        Err(Error::Api {
            status: status.as_u16(),
            message,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CursorClient;

    #[test]
    fn endpoint_uses_default_api_base() {
        let client = CursorClient::builder().api_key("test").build().unwrap();
        let url = client.endpoint("/v0/agents").unwrap();
        assert_eq!(url.as_str(), "https://api.cursor.com/v0/agents");
    }

    #[test]
    fn endpoint_preserves_path_prefix() {
        let client = CursorClient::builder()
            .api_key("test")
            .api_base("https://example.com/proxy")
            .build()
            .unwrap();
        let url = client.endpoint("/v0/agents").unwrap();
        assert_eq!(url.as_str(), "https://example.com/proxy/v0/agents");
    }
}
