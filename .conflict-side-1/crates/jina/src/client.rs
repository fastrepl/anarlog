#[derive(Clone, Default)]
pub struct JinaClientBuilder {
    api_key: Option<String>,
}

#[derive(Clone)]
pub struct JinaClient {
    pub(crate) client: reqwest::Client,
}

impl JinaClient {
    pub fn builder() -> JinaClientBuilder {
        JinaClientBuilder::default()
    }
}

impl JinaClientBuilder {
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn build(self) -> Result<JinaClient, crate::Error> {
        let api_key = self.api_key.ok_or(crate::Error::MissingApiKey)?;

        let mut headers = reqwest::header::HeaderMap::new();

        let auth_str = format!("Bearer {}", api_key);
        let mut auth_value = reqwest::header::HeaderValue::from_str(&auth_str)
            .map_err(|_| crate::Error::InvalidApiKey)?;
        auth_value.set_sensitive(true);

        headers.insert(reqwest::header::AUTHORIZATION, auth_value);
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(JinaClient { client })
    }
}

pub(crate) async fn check_response(
    response: reqwest::Response,
) -> Result<reqwest::Response, crate::Error> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else {
        let status_code = status.as_u16();
        let body = response.text().await.unwrap_or_default();
        Err(crate::Error::Api(status_code, body))
    }
}
