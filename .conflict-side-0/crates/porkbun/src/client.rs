use serde::de::DeserializeOwned;

#[derive(Clone, Default)]
pub struct PorkbunClientBuilder {
    api_key: Option<String>,
    secret_api_key: Option<String>,
    api_base: Option<String>,
}

#[derive(Clone)]
pub struct PorkbunClient {
    pub(crate) client: reqwest::Client,
    pub(crate) api_base: url::Url,
    pub(crate) api_key: String,
    pub(crate) secret_api_key: String,
}

impl PorkbunClient {
    pub fn builder() -> PorkbunClientBuilder {
        PorkbunClientBuilder::default()
    }
}

impl PorkbunClientBuilder {
    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn secret_api_key(mut self, secret_api_key: impl Into<String>) -> Self {
        self.secret_api_key = Some(secret_api_key.into());
        self
    }

    pub fn build(self) -> Result<PorkbunClient, crate::Error> {
        let api_key = self.api_key.ok_or(crate::Error::MissingApiKey)?;
        let secret_api_key = self
            .secret_api_key
            .ok_or(crate::Error::MissingSecretApiKey)?;
        let api_base = self
            .api_base
            .unwrap_or_else(|| "https://api.porkbun.com".to_string());

        let client = reqwest::Client::builder().build()?;

        Ok(PorkbunClient {
            client,
            api_base: api_base.parse().map_err(|_| crate::Error::InvalidApiBase)?,
            api_key,
            secret_api_key,
        })
    }
}

pub(crate) async fn parse_response<T: DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, crate::Error> {
    let status = response.status();
    if status.is_success() {
        Ok(response.json().await?)
    } else {
        let status_code = status.as_u16();
        let body = response.text().await.unwrap_or_default();
        Err(crate::Error::Api(status_code, body))
    }
}
