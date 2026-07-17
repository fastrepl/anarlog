use reqwest::Method;
use serde::{Serialize, de::DeserializeOwned};

use crate::client::parse_text;

/// Authentication for requests against a VM's HTTPS proxy.
#[derive(Clone)]
pub enum VmAuth {
    /// `Authorization: Bearer <token>` — either exe0 or exe1.
    Bearer(String),
    /// HTTP basic; username is ignored by the proxy, password is the token.
    /// Useful for tools like `git` that can't send bearer headers.
    Basic(String),
}

impl std::fmt::Debug for VmAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bearer(_) => f.write_str("VmAuth::Bearer(<redacted>)"),
            Self::Basic(_) => f.write_str("VmAuth::Basic(<redacted>)"),
        }
    }
}

/// HTTPS client for a single VM's auth-proxy (`https://<vm>.exe.xyz`).
///
/// The proxy verifies the bearer/basic token with exe.dev and, on success,
/// injects `X-ExeDev-UserID`, `X-ExeDev-Email`, and `X-ExeDev-Token-Ctx` into
/// the request forwarded to your VM's HTTP server.
#[derive(Clone)]
pub struct VmHttpClient {
    client: reqwest::Client,
    base: url::Url,
    auth: VmAuth,
}

impl std::fmt::Debug for VmHttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmHttpClient")
            .field("base", &self.base.as_str())
            .field("auth", &self.auth)
            .finish()
    }
}

impl VmHttpClient {
    /// Build a client pointed at `https://<vm_name>.exe.xyz`.
    pub fn for_vm(vm_name: &str, auth: VmAuth) -> Result<Self, crate::Error> {
        let base = format!("https://{vm_name}.exe.xyz")
            .parse()
            .map_err(|_| crate::Error::InvalidVmUrl)?;
        Self::with_base(base, auth)
    }

    /// Build a client pointed at an explicit VM URL (useful for custom hosts).
    pub fn with_base(base: url::Url, auth: VmAuth) -> Result<Self, crate::Error> {
        let client = reqwest::Client::builder().build()?;
        Ok(Self { client, base, auth })
    }

    pub fn base(&self) -> &url::Url {
        &self.base
    }

    fn url_for(&self, path: &str) -> Result<url::Url, crate::Error> {
        let trimmed = path.trim_start_matches('/');
        self.base
            .join(trimmed)
            .map_err(|_| crate::Error::InvalidVmUrl)
    }

    fn authed(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.auth {
            VmAuth::Bearer(token) => builder.bearer_auth(token),
            VmAuth::Basic(password) => builder.basic_auth("exe", Some(password)),
        }
    }

    pub async fn request(
        &self,
        method: Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = self.url_for(path)?;
        Ok(self.authed(self.client.request(method, url)))
    }

    pub async fn get_text(&self, path: &str) -> Result<String, crate::Error> {
        let resp = self.request(Method::GET, path).await?.send().await?;
        parse_text(resp).await
    }

    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, crate::Error> {
        let text = self.get_text(path).await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn post_json<B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<String, crate::Error> {
        let resp = self
            .request(Method::POST, path)
            .await?
            .json(body)
            .send()
            .await?;
        parse_text(resp).await
    }

    pub async fn post_json_for<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, crate::Error> {
        let text = self.post_json(path, body).await?;
        Ok(serde_json::from_str(&text)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_vm_builds_https_base() {
        let c = VmHttpClient::for_vm("claw-abc", VmAuth::Bearer("t".into())).unwrap();
        assert_eq!(c.base().as_str(), "https://claw-abc.exe.xyz/");
    }

    #[test]
    fn joins_paths_without_stripping_base() {
        let c = VmHttpClient::for_vm("claw-abc", VmAuth::Bearer("t".into())).unwrap();
        let url = c.url_for("/v1/health").unwrap();
        assert_eq!(url.as_str(), "https://claw-abc.exe.xyz/v1/health");
        let url = c.url_for("v1/health").unwrap();
        assert_eq!(url.as_str(), "https://claw-abc.exe.xyz/v1/health");
    }

    #[test]
    fn vm_auth_redacts_in_debug() {
        let b = VmAuth::Bearer("secret".into());
        assert!(!format!("{b:?}").contains("secret"));
        let b = VmAuth::Basic("secret".into());
        assert!(!format!("{b:?}").contains("secret"));
    }
}
