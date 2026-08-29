use crate::proxy::{NangoProxy, OwnedNangoProxy};

pub struct NangoHttpClient<'a> {
    proxy: NangoProxy<'a>,
}

impl<'a> NangoHttpClient<'a> {
    pub fn new(proxy: NangoProxy<'a>) -> Self {
        Self { proxy }
    }
}

async fn proxy_bytes(
    response: reqwest::Response,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    Ok(crate::client::response_bytes(response).await?)
}

impl<'a> anlg_http::HttpClient for NangoHttpClient<'a> {
    async fn get(&self, path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.get(path)?.send().await?;
        proxy_bytes(response).await
    }

    async fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.post(path, body, content_type)?.send().await?;
        proxy_bytes(response).await
    }

    async fn put(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let json_value: serde_json::Value = serde_json::from_slice(&body)?;
        let response = self.proxy.put(path, &json_value)?.send().await?;
        proxy_bytes(response).await
    }

    async fn patch(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let json_value: serde_json::Value = serde_json::from_slice(&body)?;
        let response = self.proxy.patch(path, &json_value)?.send().await?;
        proxy_bytes(response).await
    }

    async fn delete(
        &self,
        path: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.delete(path)?.send().await?;
        proxy_bytes(response).await
    }
}

#[derive(Clone)]
pub struct OwnedNangoHttpClient {
    proxy: OwnedNangoProxy,
}

impl OwnedNangoHttpClient {
    pub fn new(proxy: OwnedNangoProxy) -> Self {
        Self { proxy }
    }

    pub fn with_retries(mut self, retries: u32) -> Self {
        self.proxy = self.proxy.retries(retries);
        self
    }

    pub fn with_base_url_override(mut self, base_url: impl Into<String>) -> Self {
        self.proxy = self.proxy.base_url_override(base_url);
        self
    }

    /// For providers that need request headers the `HttpClient` trait cannot
    /// express (e.g. `Notion-Version`).
    pub fn into_proxy(self) -> OwnedNangoProxy {
        self.proxy
    }
}

impl anlg_http::HttpClient for OwnedNangoHttpClient {
    async fn get(&self, path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.get(path)?.send().await?;
        proxy_bytes(response).await
    }

    async fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.post(path, body, content_type)?.send().await?;
        proxy_bytes(response).await
    }

    async fn put(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let json_value: serde_json::Value = serde_json::from_slice(&body)?;
        let response = self.proxy.put(path, &json_value)?.send().await?;
        proxy_bytes(response).await
    }

    async fn patch(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let json_value: serde_json::Value = serde_json::from_slice(&body)?;
        let response = self.proxy.patch(path, &json_value)?.send().await?;
        proxy_bytes(response).await
    }

    async fn delete(
        &self,
        path: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.delete(path)?.send().await?;
        proxy_bytes(response).await
    }
}
