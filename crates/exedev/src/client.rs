use serde::de::DeserializeOwned;

use crate::token::{Exe0Token, NAMESPACE_API};

pub const DEFAULT_API_BASE: &str = "https://exe.dev";

#[derive(Default)]
pub struct ExedevClientBuilder {
    api_base: Option<String>,
    token: Option<String>,
    signing_key: Option<String>,
    permissions: Option<serde_json::Value>,
    http: Option<reqwest::Client>,
}

/// Thin HTTPS client for `POST https://exe.dev/exec`.
///
/// Clone is cheap: the underlying `reqwest::Client` is an `Arc`.
#[derive(Clone)]
pub struct ExedevClient {
    pub(crate) client: reqwest::Client,
    pub(crate) api_base: url::Url,
    pub(crate) token: String,
}

impl ExedevClient {
    pub fn builder() -> ExedevClientBuilder {
        ExedevClientBuilder::default()
    }

    pub fn api_base(&self) -> &url::Url {
        &self.api_base
    }
}

impl ExedevClientBuilder {
    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Raw OpenSSH PEM private key used to mint an exe0 token at build time.
    pub fn signing_key(mut self, signing_key: impl Into<String>) -> Self {
        self.signing_key = Some(signing_key.into());
        self
    }

    /// Permissions JSON used when minting an exe0 token from a signing key.
    ///
    /// The server's default `cmds` list does not include `rm`, `stat`, `rename`,
    /// `tag`, `resize`, `restart`, or `share` subcommands; pass an explicit
    /// `cmds` array if the caller needs them.
    pub fn permissions(mut self, permissions: serde_json::Value) -> Self {
        self.permissions = Some(permissions);
        self
    }

    /// Provide a pre-configured reqwest client (e.g. with custom timeouts).
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    pub fn build(self) -> Result<ExedevClient, crate::Error> {
        let api_base = self
            .api_base
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string());
        let api_base: url::Url = api_base.parse().map_err(|_| crate::Error::InvalidApiBase)?;

        let token = match (self.token, self.signing_key, self.permissions) {
            (Some(t), _, _) => t,
            (None, Some(key), Some(perms)) => {
                Exe0Token::mint(&perms, &key, NAMESPACE_API)?.into_string()
            }
            _ => return Err(crate::Error::MissingToken),
        };

        let client = match self.http {
            Some(c) => c,
            None => reqwest::Client::builder().build()?,
        };
        Ok(ExedevClient {
            client,
            api_base,
            token,
        })
    }
}

impl ExedevClient {
    /// Execute a raw command string against `/exec` and return the text body.
    ///
    /// Callers generally want the typed helpers in `commands`; use this only
    /// as an escape hatch for commands the SDK has not yet wrapped.
    pub async fn exec_raw(&self, command: &str) -> Result<String, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/exec");

        let response = self
            .client
            .post(url)
            .bearer_auth(&self.token)
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(command.to_owned())
            .send()
            .await?;

        parse_text(response).await
    }

    pub async fn exec_json<T: DeserializeOwned>(&self, command: &str) -> Result<T, crate::Error> {
        let text = self.exec_raw(command).await?;
        Ok(serde_json::from_str(&text)?)
    }
}

pub(crate) async fn parse_text(response: reqwest::Response) -> Result<String, crate::Error> {
    let status = response.status();
    let body = response.text().await?;
    if status.is_success() {
        Ok(body)
    } else {
        Err(crate::Error::from_status(status.as_u16(), body))
    }
}
