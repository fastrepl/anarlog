use crate::proxy::{NangoProxy, OwnedNangoProxy};

pub struct NangoHttpClient<'a> {
    proxy: NangoProxy<'a>,
}

impl<'a> NangoHttpClient<'a> {
    pub fn new(proxy: NangoProxy<'a>) -> Self {
        Self { proxy }
    }
}

impl<'a> hypr_http::HttpClient for NangoHttpClient<'a> {
    async fn get(&self, path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.get(path)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.post(path, body, content_type)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn put(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let json_value: serde_json::Value = serde_json::from_slice(&body)?;
        let response = self.proxy.put(path, &json_value)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn patch(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let json_value: serde_json::Value = serde_json::from_slice(&body)?;
        let response = self.proxy.patch(path, &json_value)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn delete(
        &self,
        path: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.delete(path)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
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
}

impl hypr_http::HttpClient for OwnedNangoHttpClient {
    async fn get(&self, path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.get(path)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.post(path, body, content_type)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn put(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let json_value: serde_json::Value = serde_json::from_slice(&body)?;
        let response = self.proxy.put(path, &json_value)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn patch(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let json_value: serde_json::Value = serde_json::from_slice(&body)?;
        let response = self.proxy.patch(path, &json_value)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn delete(
        &self,
        path: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.proxy.delete(path)?.send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        Ok(bytes.to_vec())
    }
}
