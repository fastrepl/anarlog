use serde::de::DeserializeOwned;

#[derive(Clone, Default)]
pub struct ExaClientBuilder {
    api_key: Option<String>,
    api_base: Option<String>,
}

#[derive(Clone)]
pub struct ExaClient {
    pub(crate) client: reqwest::Client,
    pub(crate) api_base: url::Url,
}

impl ExaClient {
    pub fn builder() -> ExaClientBuilder {
        ExaClientBuilder::default()
    }
}

impl ExaClientBuilder {
    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn build(self) -> Result<ExaClient, crate::Error> {
        let api_key = self.api_key.ok_or(crate::Error::MissingApiKey)?;
        let api_base = self
            .api_base
            .unwrap_or_else(|| "https://api.exa.ai".to_string());

        let mut headers = reqwest::header::HeaderMap::new();

        let auth_str = format!("Bearer {}", api_key);
        let mut auth_value = reqwest::header::HeaderValue::from_str(&auth_str)
            .map_err(|_| crate::Error::InvalidApiKey)?;
        auth_value.set_sensitive(true);

        headers.insert(reqwest::header::AUTHORIZATION, auth_value);
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(ExaClient {
            client,
            api_base: api_base.parse().map_err(|_| crate::Error::InvalidApiBase)?,
        })
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

pub(crate) async fn parse_response<T: DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, crate::Error> {
    let response = check_response(response).await?;
    Ok(response.json().await?)
}
