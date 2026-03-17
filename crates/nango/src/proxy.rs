use crate::client::NangoClient;

#[derive(Clone)]
pub struct NangoProxy<'a> {
    nango: &'a NangoClient,
    integration_id: String,
    connection_id: String,
    retries: Option<u32>,
    retry_on: Option<Vec<u16>>,
    base_url_override: Option<String>,
    decompress: Option<bool>,
}

impl<'a> NangoProxy<'a> {
    pub(crate) fn new(
        nango: &'a NangoClient,
        integration_id: String,
        connection_id: String,
    ) -> Self {
        Self {
            nango,
            integration_id,
            connection_id,
            retries: None,
            retry_on: None,
            base_url_override: None,
            decompress: None,
        }
    }

    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = Some(retries);
        self
    }

    pub fn retry_on(mut self, status_codes: Vec<u16>) -> Self {
        self.retry_on = Some(status_codes);
        self
    }

    pub fn base_url_override(mut self, base_url: impl Into<String>) -> Self {
        self.base_url_override = Some(base_url.into());
        self
    }

    pub fn decompress(mut self, decompress: bool) -> Self {
        self.decompress = Some(decompress);
        self
    }

    fn apply_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        apply_proxy_headers(
            builder,
            &self.connection_id,
            &self.integration_id,
            self.retries,
            self.retry_on.as_ref(),
            self.base_url_override.as_deref(),
            self.decompress,
        )
    }

    pub fn get(
        &self,
        path: impl std::fmt::Display,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.nango.api_base, path)?;
        Ok(self.apply_headers(self.nango.client.get(url)))
    }

    pub fn post(
        &self,
        path: impl std::fmt::Display,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.nango.api_base, path)?;
        Ok(self.apply_headers(
            self.nango
                .client
                .post(url)
                .header("Content-Type", content_type)
                .body(body),
        ))
    }

    pub fn put<T: serde::Serialize + ?Sized>(
        &self,
        path: impl std::fmt::Display,
        data: &T,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.nango.api_base, path)?;
        Ok(self.apply_headers(
            self.nango
                .client
                .put(url)
                .header("Content-Type", "application/json")
                .json(data),
        ))
    }

    pub fn patch<T: serde::Serialize + ?Sized>(
        &self,
        path: impl std::fmt::Display,
        data: &T,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.nango.api_base, path)?;
        Ok(self.apply_headers(
            self.nango
                .client
                .patch(url)
                .header("Content-Type", "application/json")
                .json(data),
        ))
    }

    pub fn delete(
        &self,
        path: impl std::fmt::Display,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.nango.api_base, path)?;
        Ok(self.apply_headers(self.nango.client.delete(url)))
    }
}

#[derive(Clone)]
pub struct OwnedNangoProxy {
    client: reqwest::Client,
    api_base: url::Url,
    integration_id: String,
    connection_id: String,
    retries: Option<u32>,
    retry_on: Option<Vec<u16>>,
    base_url_override: Option<String>,
    decompress: Option<bool>,
}

impl OwnedNangoProxy {
    pub fn new(nango: &NangoClient, integration_id: String, connection_id: String) -> Self {
        Self {
            client: nango.client.clone(),
            api_base: nango.api_base.clone(),
            integration_id,
            connection_id,
            retries: Some(3),
            retry_on: Some(vec![429, 500, 502, 503, 504]),
            base_url_override: None,
            decompress: None,
        }
    }

    fn apply_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        apply_proxy_headers(
            builder,
            &self.connection_id,
            &self.integration_id,
            self.retries,
            self.retry_on.as_ref(),
            self.base_url_override.as_deref(),
            self.decompress,
        )
    }

    pub fn get(
        &self,
        path: impl std::fmt::Display,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.api_base, path)?;
        Ok(self.apply_headers(self.client.get(url)))
    }

    pub fn post(
        &self,
        path: impl std::fmt::Display,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.api_base, path)?;
        Ok(self.apply_headers(
            self.client
                .post(url)
                .header("Content-Type", content_type)
                .body(body),
        ))
    }

    pub fn put<T: serde::Serialize + ?Sized>(
        &self,
        path: impl std::fmt::Display,
        data: &T,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.api_base, path)?;
        Ok(self.apply_headers(
            self.client
                .put(url)
                .header("Content-Type", "application/json")
                .json(data),
        ))
    }

    pub fn patch<T: serde::Serialize + ?Sized>(
        &self,
        path: impl std::fmt::Display,
        data: &T,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.api_base, path)?;
        Ok(self.apply_headers(
            self.client
                .patch(url)
                .header("Content-Type", "application/json")
                .json(data),
        ))
    }

    pub fn delete(
        &self,
        path: impl std::fmt::Display,
    ) -> Result<reqwest::RequestBuilder, crate::Error> {
        let url = make_proxy_url(&self.api_base, path)?;
        Ok(self.apply_headers(self.client.delete(url)))
    }
}

fn apply_proxy_headers(
    builder: reqwest::RequestBuilder,
    connection_id: &str,
    integration_id: &str,
    retries: Option<u32>,
    retry_on: Option<&Vec<u16>>,
    base_url_override: Option<&str>,
    decompress: Option<bool>,
) -> reqwest::RequestBuilder {
    let mut builder = builder
        .header("Connection-Id", connection_id)
        .header("Provider-Config-Key", integration_id);

    if let Some(retries) = retries {
        builder = builder.header("Retries", retries.to_string());
    }

    if let Some(retry_on) = retry_on {
        let codes: Vec<String> = retry_on.iter().map(|c| c.to_string()).collect();
        builder = builder.header("Retry-On", codes.join(","));
    }

    if let Some(base_url) = base_url_override {
        builder = builder.header("Base-Url-Override", base_url);
    }

    if let Some(decompress) = decompress {
        builder = builder.header("Decompress", decompress.to_string());
    }

    builder
}

fn make_proxy_url(base: &url::Url, path: impl std::fmt::Display) -> Result<url::Url, crate::Error> {
    let mut url = base.clone();
    let path_str = path.to_string();

    let (path_part, query_part) = match path_str.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (path_str.as_str(), None),
    };

    url.path_segments_mut()
        .map_err(|_| crate::Error::InvalidUrl)?
        .push("proxy")
        .extend(path_part.split('/').filter(|s| !s.is_empty()));

    if let Some(query) = query_part {
        url.set_query(Some(query));
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_proxy_url() {
        let base = "https://api.nango.dev".parse().unwrap();

        assert_eq!(
            make_proxy_url(&base, "/users").unwrap().to_string(),
            "https://api.nango.dev/proxy/users"
        );
        assert_eq!(
            make_proxy_url(&base, "users/123").unwrap().to_string(),
            "https://api.nango.dev/proxy/users/123"
        );
        assert_eq!(
            make_proxy_url(&base, "users/123?foo=bar")
                .unwrap()
                .to_string(),
            "https://api.nango.dev/proxy/users/123?foo=bar"
        );
        assert_eq!(
            make_proxy_url(&base, "users?page=1&limit=10")
                .unwrap()
                .to_string(),
            "https://api.nango.dev/proxy/users?page=1&limit=10"
        );
    }
}
