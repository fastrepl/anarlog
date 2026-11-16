use std::time::Duration;

use desktop_runtime::{Result, ServiceError};
use futures::StreamExt;
use reqwest::{Client, Method, StatusCode, Url, header::HeaderValue};
use serde_json::Value;

#[derive(Clone)]
pub struct CloudConfig {
    pub supabase: Url,
    pub api: Url,
    pub web: Url,
    pub anon_key: String,
    pub callback: Url,
    pub credential_namespace: String,
    pub device_fingerprint: String,
    pub device_name: String,
}

impl CloudConfig {
    pub fn validate(&self) -> Result<()> {
        for url in [&self.supabase, &self.api, &self.web] {
            let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
            if (url.scheme() != "https" && !(url.scheme() == "http" && local))
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(failure(
                    "Cloud endpoints require HTTPS (or loopback fixtures)",
                ));
            }
        }
        if self.credential_namespace.is_empty()
            || self.anon_key.is_empty()
            || self.device_fingerprint.is_empty()
            || self.callback.query().is_some()
            || self.callback.fragment().is_some()
            || matches!(self.callback.scheme(), "http" | "https" | "file")
        {
            return Err(failure("Invalid cloud configuration"));
        }
        Ok(())
    }

    pub fn web_flow(&self, path: &str) -> Result<Url> {
        let mut url = self
            .web
            .join(path)
            .map_err(|_| failure("Invalid web route"))?;
        url.query_pairs_mut()
            .append_pair("flow", "desktop")
            .append_pair("scheme", self.callback.scheme());
        Ok(url)
    }
}

#[derive(Clone)]
pub(crate) struct Transport {
    pub config: CloudConfig,
    pub client: Client,
}

pub(crate) struct Response {
    pub status: StatusCode,
    pub body: Value,
}

pub(super) fn check_directory_size(
    existing: usize,
    batch: &[Value],
    bytes: &mut usize,
) -> Result<()> {
    *bytes += serde_json::to_vec(batch)
        .map_err(|_| failure("Invalid directory page"))?
        .len();
    if existing + batch.len() > 16_384 || *bytes > 16 * 1024 * 1024 {
        return Err(failure("Cloud directory exceeds the response limit"));
    }
    Ok(())
}

impl Transport {
    pub fn new(config: CloudConfig) -> Result<Self> {
        config.validate()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| failure("Could not initialize cloud transport"))?;
        Ok(Self { config, client })
    }

    pub async fn send(
        &self,
        method: Method,
        url: Url,
        bearer: Option<&str>,
        body: Option<&Value>,
        headers: &[(&str, String)],
    ) -> Result<Response> {
        let mut request = self
            .client
            .request(method, url)
            .header("Accept", "application/json");
        if let Some(bearer) = bearer {
            let mut header = HeaderValue::from_str(&format!("Bearer {bearer}"))
                .map_err(|_| failure("Invalid authorization credential"))?;
            header.set_sensitive(true);
            request = request.header("Authorization", header);
        }
        for (key, value) in headers {
            request = request.header(*key, value);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .map_err(|_| failure("Cloud request failed; retry when online"))?;
        let status = response.status();
        const LIMIT: usize = 16 * 1024 * 1024;
        if response
            .content_length()
            .is_some_and(|length| length > LIMIT as u64)
        {
            return Err(failure("Cloud response exceeds size limit"));
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| failure("Cloud response interrupted"))?;
            if bytes.len().saturating_add(chunk.len()) > LIMIT {
                return Err(failure("Cloud response exceeds size limit"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).map_err(|_| failure("Malformed cloud response"))?
        };
        Ok(Response { status, body })
    }

    pub fn checked(response: Response) -> Result<Value> {
        match response.status {
            status if status.is_success() => Ok(response.body),
            StatusCode::UNAUTHORIZED => Err(failure("Your session has expired. Sign in again")),
            StatusCode::FORBIDDEN => Err(failure("This account does not have permission")),
            StatusCode::CONFLICT => Err(ServiceError::Conflict),
            StatusCode::TOO_MANY_REQUESTS => {
                Err(failure("Too many requests. Please try again later"))
            }
            _ => Err(failure(
                "Cloud service rejected the request; local data was retained",
            )),
        }
    }
}

pub(crate) fn failure(message: &str) -> ServiceError {
    ServiceError::Failed(message.to_owned().into())
}

pub(crate) fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| failure("Cloud response is missing a required field"))
}

pub(crate) fn first(value: &Value) -> Result<&Value> {
    value
        .as_array()
        .and_then(|rows| rows.first())
        .ok_or_else(|| failure("Cloud response is missing a result"))
}

pub(crate) fn uuid(value: &str) -> Result<&str> {
    uuid::Uuid::parse_str(value).map_err(|_| failure("Invalid identifier"))?;
    Ok(value)
}
