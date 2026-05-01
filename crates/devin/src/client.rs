use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue};

use crate::{
    CursorPage, Error, ListSessionMessagesRequest, ListSessionsRequest, Session, SessionMessage,
};

const DEFAULT_API_BASE: &str = "https://api.devin.ai/";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct DevinClientBuilder {
    api_key: Option<String>,
    api_base: Option<String>,
    client: Option<reqwest::Client>,
    timeout: Option<Duration>,
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

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Result<DevinClient, Error> {
        let api_key = self.api_key.ok_or(Error::MissingApiKey)?;

        let _ = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| Error::InvalidApiKey)?;

        let client = match self.client {
            Some(client) => client,
            None => {
                let mut headers = HeaderMap::new();
                headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
                headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

                reqwest::Client::builder()
                    .default_headers(headers)
                    .timeout(self.timeout.unwrap_or(DEFAULT_TIMEOUT))
                    .build()?
            }
        };

        let raw_base = self
            .api_base
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string());
        let api_base = normalize_base(&raw_base)?;

        Ok(DevinClient {
            client,
            api_base,
            api_key,
        })
    }
}

#[derive(Clone)]
pub struct DevinClient {
    client: reqwest::Client,
    api_base: url::Url,
    api_key: String,
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
        let mut url = self.endpoint(&format!("v3/organizations/{org_id}/sessions"))?;
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
                    pairs.append_pair("origins", origin.as_str());
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
        let url = self.endpoint(&format!("v3/organizations/{org_id}/sessions/{devin_id}"))?;
        self.send(self.client.get(url)).await
    }

    pub async fn list_session_messages(
        &self,
        org_id: &str,
        devin_id: &str,
        req: ListSessionMessagesRequest,
    ) -> Result<CursorPage<SessionMessage>, Error> {
        let mut url = self.endpoint(&format!(
            "v3/organizations/{org_id}/sessions/{devin_id}/messages"
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
        let mut url = self.endpoint(&format!("v3/organizations/{org_id}/sessions/{devin_id}"))?;
        if archive {
            url.query_pairs_mut().append_pair("archive", "true");
        }

        self.send(self.client.delete(url)).await
    }

    pub async fn archive_session(&self, org_id: &str, devin_id: &str) -> Result<Session, Error> {
        let url = self.endpoint(&format!(
            "v3/organizations/{org_id}/sessions/{devin_id}/archive"
        ))?;
        self.send(self.client.post(url)).await
    }

    fn endpoint(&self, path: &str) -> Result<url::Url, Error> {
        debug_assert!(
            !path.starts_with('/'),
            "endpoint paths must be relative to api_base",
        );
        self.api_base.join(path).map_err(Error::from)
    }

    async fn send<T>(&self, request: reqwest::RequestBuilder) -> Result<T, Error>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = request.bearer_auth(&self.api_key).send().await?;
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

fn normalize_base(raw: &str) -> Result<url::Url, Error> {
    let normalized = if raw.ends_with('/') {
        raw.to_string()
    } else {
        format!("{raw}/")
    };
    Ok(normalized.parse()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_base_adds_trailing_slash() {
        let url = normalize_base("https://api.devin.ai").unwrap();
        assert_eq!(url.as_str(), "https://api.devin.ai/");
    }

    #[test]
    fn normalize_base_preserves_trailing_slash() {
        let url = normalize_base("https://api.devin.ai/").unwrap();
        assert_eq!(url.as_str(), "https://api.devin.ai/");
    }

    #[test]
    fn normalize_base_with_path_prefix_no_trailing_slash() {
        let url = normalize_base("https://gateway.corp/devin").unwrap();
        assert_eq!(url.as_str(), "https://gateway.corp/devin/");
    }

    #[test]
    fn endpoint_appends_to_prefixed_base() {
        let client = DevinClient::builder()
            .api_key("secret")
            .api_base("https://gateway.corp/devin")
            .build()
            .unwrap();

        let url = client.endpoint("v3/organizations/org_1/sessions").unwrap();
        assert_eq!(
            url.as_str(),
            "https://gateway.corp/devin/v3/organizations/org_1/sessions",
        );
    }
}
