mod error;
mod models;

pub use error::*;
pub use models::*;

#[derive(Clone)]
pub struct LoopClient {
    client: reqwest::Client,
    api_base: url::Url,
}

#[derive(Default)]
pub struct LoopClientBuilder {
    api_key: Option<String>,
}

impl LoopClientBuilder {
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn build(self) -> LoopClient {
        let mut headers = reqwest::header::HeaderMap::new();

        let api_key = self.api_key.unwrap();
        let auth_str = format!("Bearer {}", &api_key);
        let mut auth_value = reqwest::header::HeaderValue::from_str(&auth_str).unwrap();
        auth_value.set_sensitive(true);

        headers.insert(reqwest::header::AUTHORIZATION, auth_value);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        LoopClient {
            client,
            api_base: "https://app.loops.so".parse().unwrap(),
        }
    }
}

impl LoopClient {
    pub fn builder() -> LoopClientBuilder {
        LoopClientBuilder::default()
    }

    // https://loops.so/docs/api-reference/send-event
    pub async fn send_event(&self, event: Event) -> Result<Response, Error> {
        let url = {
            let mut url = self.api_base.clone();
            url.set_path("api/v1/events");
            url
        };

        let res = self
            .client
            .post(url)
            .json(&event)
            .send()
            .await?
            .json()
            .await?;
        Ok(res)
    }

    // https://loops.so/docs/api-reference/delete-contact
    pub async fn delete_contact_by_email(
        &self,
        email: &str,
    ) -> Result<DeleteContactResponse, Error> {
        let url = {
            let mut url = self.api_base.clone();
            url.set_path("api/v1/contacts/delete");
            url
        };

        let res = self
            .client
            .post(url)
            .json(&serde_json::json!({ "email": email }))
            .send()
            .await?
            .json()
            .await?;
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_client() -> LoopClient {
        LoopClient::builder().api_key("LOOPS_API_KEY").build()
    }

    #[tokio::test]
    async fn test_get_user() {
        let _ = get_client();
    }
}
