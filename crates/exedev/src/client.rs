use serde::de::DeserializeOwned;

use crate::token::{Exe0Token, NAMESPACE_API};

pub const DEFAULT_API_BASE: &str = "https://exe.dev";
const MAX_EXEC_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ERROR_RESPONSE_BYTES: usize = 64 * 1024;

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
    let limit = if status.is_success() {
        MAX_EXEC_RESPONSE_BYTES
    } else {
        MAX_ERROR_RESPONSE_BYTES
    };
    let (mut body, truncated) = read_text_prefix(response, limit).await?;
    if status.is_success() {
        if truncated {
            return Err(crate::Error::ResponseTooLarge { limit });
        }
        Ok(body)
    } else {
        if truncated {
            body.push_str("\n[response body truncated]");
        }
        Err(crate::Error::from_status(status.as_u16(), body))
    }
}

async fn read_text_prefix(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<(String, bool), reqwest::Error> {
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or_default()
            .min(limit),
    );
    let mut truncated = response
        .content_length()
        .is_some_and(|length| length > limit as u64);
    while let Some(chunk) = response.chunk().await? {
        let remaining = limit.saturating_sub(body.len());
        let keep = remaining.min(chunk.len());
        body.extend_from_slice(&chunk[..keep]);
        if keep < chunk.len() {
            truncated = true;
            break;
        }
        if body.len() == limit {
            if response.chunk().await?.is_some() {
                truncated = true;
            }
            break;
        }
    }
    Ok((String::from_utf8_lossy(&body).into_owned(), truncated))
}

#[cfg(test)]
mod tests {
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

    use super::read_text_prefix;

    #[tokio::test]
    async fn response_reader_only_retains_the_configured_prefix() {
        let server = MockServer::start().await;
        Mock::given(path("/large"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 17]))
            .mount(&server)
            .await;
        let response = reqwest::get(format!("{}/large", server.uri()))
            .await
            .unwrap();

        let (body, truncated) = read_text_prefix(response, 16).await.unwrap();
        assert_eq!(body, "x".repeat(16));
        assert!(truncated);
    }
}
