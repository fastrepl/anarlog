use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};

use crate::{
    CursorPage, Error, ListSessionMessagesRequest, ListSessionsRequest, Session, SessionMessage,
};

#[derive(Default)]
pub struct DevinClientBuilder {
    api_key: Option<String>,
    api_base: Option<String>,
    client: Option<reqwest::Client>,
}

impl DevinClientBuilder {
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

    pub fn build(self) -> Result<DevinClient, Error> {
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
            .unwrap_or_else(|| "https://api.devin.ai".to_string())
            .parse()?;

        Ok(DevinClient { client, api_base })
    }
}

#[derive(Clone)]
pub struct DevinClient {
    client: reqwest::Client,
    api_base: url::Url,
}

impl DevinClient {
    pub fn builder() -> DevinClientBuilder {
        DevinClientBuilder::default()
    }

    pub fn api_base(&self) -> &url::Url {
        &self.api_base
    }

    pub async fn list_sessions(
        &self,
        org_id: &str,
        req: ListSessionsRequest,
    ) -> Result<CursorPage<Session>, Error> {
        let mut url = self.endpoint(&format!("/v3/organizations/{org_id}/sessions"))?;
        {
            let mut pairs = url.query_pairs_mut();

            if let Some(after) = req.after.as_deref() {
                pairs.append_pair("after", after);
            }
            if let Some(first) = req.first {
                pairs.append_pair("first", &first.to_string());
            }
            if let Some(session_ids) = req.session_ids.as_ref() {
                for session_id in session_ids {
                    pairs.append_pair("session_ids", session_id);
                }
            }
            if let Some(created_after) = req.created_after {
                pairs.append_pair("created_after", &created_after.to_string());
            }
            if let Some(created_before) = req.created_before {
                pairs.append_pair("created_before", &created_before.to_string());
            }
            if let Some(updated_after) = req.updated_after {
                pairs.append_pair("updated_after", &updated_after.to_string());
            }
            if let Some(updated_before) = req.updated_before {
                pairs.append_pair("updated_before", &updated_before.to_string());
            }
            if let Some(tags) = req.tags.as_ref() {
                for tag in tags {
                    pairs.append_pair("tags", tag);
                }
            }
            if let Some(playbook_id) = req.playbook_id.as_deref() {
                pairs.append_pair("playbook_id", playbook_id);
            }
            if let Some(origins) = req.origins.as_ref() {
                for origin in origins {
                    let value = serde_json::to_string(origin)
                        .expect("serializing SessionOrigin should not fail");
                    pairs.append_pair("origins", value.trim_matches('"'));
                }
            }
            if let Some(schedule_id) = req.schedule_id.as_deref() {
                pairs.append_pair("schedule_id", schedule_id);
            }
            if let Some(user_ids) = req.user_ids.as_ref() {
                for user_id in user_ids {
                    pairs.append_pair("user_ids", user_id);
                }
            }
            if let Some(service_user_ids) = req.service_user_ids.as_ref() {
                for service_user_id in service_user_ids {
                    pairs.append_pair("service_user_ids", service_user_id);
                }
            }
        }

        self.send(self.client.get(url)).await
    }

    pub async fn get_session(&self, org_id: &str, devin_id: &str) -> Result<Session, Error> {
        let url = self.endpoint(&format!("/v3/organizations/{org_id}/sessions/{devin_id}"))?;
        self.send(self.client.get(url)).await
    }

    pub async fn list_session_messages(
        &self,
        org_id: &str,
        devin_id: &str,
        req: ListSessionMessagesRequest,
    ) -> Result<CursorPage<SessionMessage>, Error> {
        let mut url = self.endpoint(&format!(
            "/v3/organizations/{org_id}/sessions/{devin_id}/messages"
        ))?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(after) = req.after.as_deref() {
                pairs.append_pair("after", after);
            }
            if let Some(first) = req.first {
                pairs.append_pair("first", &first.to_string());
            }
        }

        self.send(self.client.get(url)).await
    }

    pub async fn terminate_session(
        &self,
        org_id: &str,
        devin_id: &str,
        archive: bool,
    ) -> Result<Session, Error> {
        let mut url = self.endpoint(&format!("/v3/organizations/{org_id}/sessions/{devin_id}"))?;
        if archive {
            url.query_pairs_mut().append_pair("archive", "true");
        }

        self.send(self.client.delete(url)).await
    }

    pub async fn archive_session(&self, org_id: &str, devin_id: &str) -> Result<Session, Error> {
        let url = self.endpoint(&format!(
            "/v3/organizations/{org_id}/sessions/{devin_id}/archive"
        ))?;
        self.send(self.client.post(url)).await
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
