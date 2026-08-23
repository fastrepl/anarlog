use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};

use crate::{
    Agent, AgentMessage, CreateAgentRequest, CreateMessageRequest, Error, IdResponse,
    ListAgentsRequest, ListAgentsResponse, ListMessagesRequest, ListMessagesResponse,
    UpdateAgentRequest,
};

#[derive(Default)]
pub struct ApiAgentClientBuilder {
    api_key: Option<String>,
    api_base: Option<String>,
    client: Option<reqwest::Client>,
}

impl ApiAgentClientBuilder {
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

    pub fn build(self) -> Result<ApiAgentClient, Error> {
        let api_key = self.api_key.ok_or(Error::MissingApiKey)?;
        let mut headers = HeaderMap::new();

        let mut auth_value = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| Error::InvalidApiKey)?;
        auth_value.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth_value);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = match self.client {
            Some(client) => client,
            None => reqwest::Client::builder()
                .default_headers(headers)
                .build()?,
        };

        let api_base = self
            .api_base
            .unwrap_or_else(|| "https://api.char.com".to_string())
            .parse()?;

        Ok(ApiAgentClient { client, api_base })
    }
}

#[derive(Clone)]
pub struct ApiAgentClient {
    client: reqwest::Client,
    api_base: url::Url,
}

impl ApiAgentClient {
    pub fn builder() -> ApiAgentClientBuilder {
        ApiAgentClientBuilder::default()
    }

    pub fn api_base(&self) -> &url::Url {
        &self.api_base
    }

    pub async fn list_codex_agents(
        &self,
        req: ListAgentsRequest,
    ) -> Result<ListAgentsResponse, Error> {
        let mut url = self.endpoint("/v1/codex/agents")?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(limit) = req.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(cursor) = req.cursor.as_deref() {
                pairs.append_pair("cursor", cursor);
            }
            if let Some(status) = req.status {
                let value = serde_json::to_string(&status)
                    .expect("serializing AgentStatus should not fail");
                pairs.append_pair("status", value.trim_matches('"'));
            }
        }

        self.send(self.client.get(url)).await
    }

    pub async fn get_codex_agent(&self, id: &str) -> Result<Agent, Error> {
        let url = self.endpoint(&format!("/v1/codex/agents/{id}"))?;
        self.send(self.client.get(url)).await
    }

    pub async fn create_codex_agent(&self, req: &CreateAgentRequest) -> Result<Agent, Error> {
        let url = self.endpoint("/v1/codex/agents")?;
        self.send(self.client.post(url).json(req)).await
    }

    pub async fn update_codex_agent(
        &self,
        id: &str,
        req: &UpdateAgentRequest,
    ) -> Result<Agent, Error> {
        let url = self.endpoint(&format!("/v1/codex/agents/{id}"))?;
        self.send(self.client.patch(url).json(req)).await
    }

    pub async fn delete_codex_agent(&self, id: &str) -> Result<IdResponse, Error> {
        let url = self.endpoint(&format!("/v1/codex/agents/{id}"))?;
        self.send(self.client.delete(url)).await
    }

    pub async fn list_codex_messages(
        &self,
        id: &str,
        req: ListMessagesRequest,
    ) -> Result<ListMessagesResponse, Error> {
        let mut url = self.endpoint(&format!("/v1/codex/agents/{id}/messages"))?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(limit) = req.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(cursor) = req.cursor.as_deref() {
                pairs.append_pair("cursor", cursor);
            }
        }

        self.send(self.client.get(url)).await
    }

    pub async fn create_codex_message(
        &self,
        id: &str,
        req: &CreateMessageRequest,
    ) -> Result<AgentMessage, Error> {
        let url = self.endpoint(&format!("/v1/codex/agents/{id}/messages"))?;
        self.send(self.client.post(url).json(req)).await
    }

    pub async fn list_devin_agents(
        &self,
        req: ListAgentsRequest,
    ) -> Result<ListAgentsResponse, Error> {
        let mut url = self.endpoint("/v1/devin/agents")?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(limit) = req.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(cursor) = req.cursor.as_deref() {
                pairs.append_pair("cursor", cursor);
            }
            if let Some(status) = req.status {
                let value = serde_json::to_string(&status)
                    .expect("serializing AgentStatus should not fail");
                pairs.append_pair("status", value.trim_matches('"'));
            }
        }

        self.send(self.client.get(url)).await
    }

    pub async fn get_devin_agent(&self, id: &str) -> Result<Agent, Error> {
        let url = self.endpoint(&format!("/v1/devin/agents/{id}"))?;
        self.send(self.client.get(url)).await
    }

    pub async fn create_devin_agent(&self, req: &CreateAgentRequest) -> Result<Agent, Error> {
        let url = self.endpoint("/v1/devin/agents")?;
        self.send(self.client.post(url).json(req)).await
    }

    pub async fn update_devin_agent(
        &self,
        id: &str,
        req: &UpdateAgentRequest,
    ) -> Result<Agent, Error> {
        let url = self.endpoint(&format!("/v1/devin/agents/{id}"))?;
        self.send(self.client.patch(url).json(req)).await
    }

    pub async fn delete_devin_agent(&self, id: &str) -> Result<IdResponse, Error> {
        let url = self.endpoint(&format!("/v1/devin/agents/{id}"))?;
        self.send(self.client.delete(url)).await
    }

    pub async fn list_devin_messages(
        &self,
        id: &str,
        req: ListMessagesRequest,
    ) -> Result<ListMessagesResponse, Error> {
        let mut url = self.endpoint(&format!("/v1/devin/agents/{id}/messages"))?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(limit) = req.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(cursor) = req.cursor.as_deref() {
                pairs.append_pair("cursor", cursor);
            }
        }

        self.send(self.client.get(url)).await
    }

    pub async fn create_devin_message(
        &self,
        id: &str,
        req: &CreateMessageRequest,
    ) -> Result<AgentMessage, Error> {
        let url = self.endpoint(&format!("/v1/devin/agents/{id}/messages"))?;
        self.send(self.client.post(url).json(req)).await
    }

    fn endpoint(&self, path: &str) -> Result<url::Url, Error> {
        self.api_base.join(path).map_err(Error::from)
    }

    async fn send<T>(&self, request: reqwest::RequestBuilder) -> Result<T, Error>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = request.send().await?;
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
