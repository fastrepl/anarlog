use std::pin::Pin;

use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use tokio::time::{Duration, sleep};

use crate::error::Error;
use crate::types::{ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse};
use crate::types::{ResponsesRequest, ResponsesResponse, ResponsesStreamOutputItem};

const MAX_SSE_LINE_BYTES: usize = 1024 * 1024;
const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct Client {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

#[derive(serde::Serialize)]
struct RequestBody<'a, T> {
    #[serde(flatten)]
    request: &'a T,
    stream: bool,
}

impl Client {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    pub async fn chat_completion(
        &self,
        req: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, Error> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = RequestBody {
            request: req,
            stream: false,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let message = error_body(resp).await;
            return Err(Error::Api { status, message });
        }

        Ok(resp.json().await?)
    }

    pub async fn chat_completion_stream(
        &self,
        req: &ChatCompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, Error>> + Send>>, Error> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = RequestBody {
            request: req,
            stream: true,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let message = error_body(resp).await;
            return Err(Error::Api { status, message });
        }

        let byte_stream = resp.bytes_stream();
        Ok(Box::pin(parse_sse_stream(byte_stream)))
    }

    pub async fn responses(&self, req: &ResponsesRequest) -> Result<ResponsesResponse, Error> {
        let url = format!("{}/responses", self.base_url);
        let body = RequestBody {
            request: req,
            stream: false,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let message = error_body(resp).await;
            return Err(Error::Api { status, message });
        }

        Ok(resp.json().await?)
    }

    pub async fn responses_stream(
        &self,
        req: &ResponsesRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ResponsesStreamOutputItem, Error>> + Send>>, Error>
    {
        let url = format!("{}/responses", self.base_url);
        let body = RequestBody {
            request: req,
            stream: true,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let message = error_body(resp).await;
            return Err(Error::Api { status, message });
        }

        let byte_stream = resp.bytes_stream();
        Ok(Box::pin(parse_responses_sse_stream(byte_stream)))
    }

    pub async fn generation_total_cost_with_retry(
        &self,
        generation_id: &str,
        max_attempts: usize,
    ) -> Result<Option<f64>, Error> {
        if !generation_id.starts_with("gen-") {
            return Ok(None);
        }

        let max_attempts = max_attempts.max(1);
        for attempt in 1..=max_attempts {
            match self.generation_total_cost(generation_id).await {
                Ok(cost) => return Ok(cost),
                Err(Error::Api { status, .. })
                    if (status == 429 || status >= 500) && attempt < max_attempts =>
                {
                    sleep(Duration::from_millis((attempt as u64) * 200)).await;
                }
                Err(err) => return Err(err),
            }
        }

        Ok(None)
    }

    async fn generation_total_cost(&self, generation_id: &str) -> Result<Option<f64>, Error> {
        #[derive(Deserialize)]
        struct GenerationResponse {
            data: GenerationData,
        }

        #[derive(Deserialize)]
        struct GenerationData {
            total_cost: f64,
        }

        let url = format!("{}/generation?id={}", self.base_url, generation_id);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let message = error_body(resp).await;
            return Err(Error::Api { status, message });
        }

        let body: GenerationResponse = resp.json().await?;
        Ok(Some(body.data.total_cost))
    }
}

async fn error_body(mut response: reqwest::Response) -> String {
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or_default()
            .min(MAX_ERROR_BODY_BYTES),
    );
    let mut truncated = response
        .content_length()
        .is_some_and(|length| length > MAX_ERROR_BODY_BYTES as u64);
    while let Ok(Some(chunk)) = response.chunk().await {
        let remaining = MAX_ERROR_BODY_BYTES.saturating_sub(body.len());
        let keep = remaining.min(chunk.len());
        body.extend_from_slice(&chunk[..keep]);
        if keep < chunk.len() {
            truncated = true;
            break;
        }
        if body.len() == MAX_ERROR_BODY_BYTES {
            if let Ok(Some(_)) = response.chunk().await {
                truncated = true;
            }
            break;
        }
    }

    let mut body = String::from_utf8_lossy(&body).into_owned();
    if truncated {
        body.push_str("\n[response body truncated]");
    }
    body
}

fn process_line(line: &str) -> Option<Result<ChatCompletionChunk, Error>> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let data = line.strip_prefix("data: ")?;
    let data = data.trim();
    if data == "[DONE]" {
        return None;
    }
    Some(
        serde_json::from_str::<ChatCompletionChunk>(data).map_err(|e| Error::Stream(e.to_string())),
    )
}

fn parse_sse_stream(
    byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = Result<ChatCompletionChunk, Error>> + Send {
    async_stream::stream! {
        let mut buffer = Vec::<u8>::new();
        futures_util::pin_mut!(byte_stream);

        'stream: while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    yield Err(Error::Http(e));
                    break;
                }
            };

            let mut chunk_start = 0;
            while let Some(relative_newline) = chunk[chunk_start..]
                .iter()
                .position(|&byte| byte == b'\n')
            {
                let newline_pos = chunk_start + relative_newline;
                let segment = &chunk[chunk_start..newline_pos];
                if buffer.len().saturating_add(segment.len()) > MAX_SSE_LINE_BYTES {
                    yield Err(sse_line_too_long_error());
                    buffer.clear();
                    break 'stream;
                }
                buffer.extend_from_slice(segment);

                let result = match std::str::from_utf8(&buffer) {
                    Ok(s) => s,
                    Err(e) => {
                        yield Err(Error::Stream(e.to_string()));
                        buffer.clear();
                        chunk_start = newline_pos + 1;
                        continue;
                    }
                };

                let result = process_line(result);
                buffer.clear();
                if let Some(result) = result {
                    match result {
                        Ok(chunk) => yield Ok(chunk),
                        Err(e) => yield Err(e),
                    }
                }
                chunk_start = newline_pos + 1;
            }

            let remainder = &chunk[chunk_start..];
            if buffer.len().saturating_add(remainder.len()) > MAX_SSE_LINE_BYTES {
                yield Err(sse_line_too_long_error());
                buffer.clear();
                break;
            }
            buffer.extend_from_slice(remainder);
        }

        if !buffer.is_empty()
            && let Ok(line) = std::str::from_utf8(&buffer)
                && let Some(result) = process_line(line) {
                    match result {
                        Ok(chunk) => yield Ok(chunk),
                        Err(e) => yield Err(e),
                    }
                }
    }
}

fn sse_line_too_long_error() -> Error {
    Error::Stream(format!(
        "SSE line exceeds the {MAX_SSE_LINE_BYTES}-byte limit"
    ))
}

fn process_responses_line(line: &str) -> Option<Result<ResponsesStreamOutputItem, Error>> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    if let Some(data) = line.strip_prefix("data: ") {
        let data = data.trim();
        if data == "[DONE]" {
            return None;
        }
        return Some(
            serde_json::from_str::<ResponsesStreamOutputItem>(data)
                .map_err(|e| Error::Stream(e.to_string())),
        );
    }

    if line.starts_with("event:") {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
        && let Some(obj) = value.as_object()
        && obj.contains_key("type")
    {
        return Some(
            serde_json::from_str::<ResponsesStreamOutputItem>(line)
                .map_err(|e| Error::Stream(e.to_string())),
        );
    }

    None
}

fn parse_responses_sse_stream(
    byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = Result<ResponsesStreamOutputItem, Error>> + Send {
    async_stream::stream! {
        let mut buffer = Vec::<u8>::new();
        futures_util::pin_mut!(byte_stream);

        'stream: while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    yield Err(Error::Http(e));
                    break;
                }
            };

            let mut chunk_start = 0;
            while let Some(relative_newline) = chunk[chunk_start..]
                .iter()
                .position(|&byte| byte == b'\n')
            {
                let newline_pos = chunk_start + relative_newline;
                let segment = &chunk[chunk_start..newline_pos];
                if buffer.len().saturating_add(segment.len()) > MAX_SSE_LINE_BYTES {
                    yield Err(sse_line_too_long_error());
                    buffer.clear();
                    break 'stream;
                }
                buffer.extend_from_slice(segment);

                let result = match std::str::from_utf8(&buffer) {
                    Ok(s) => s,
                    Err(e) => {
                        yield Err(Error::Stream(e.to_string()));
                        buffer.clear();
                        chunk_start = newline_pos + 1;
                        continue;
                    }
                };

                let result = process_responses_line(result);
                buffer.clear();
                if let Some(result) = result {
                    match result {
                        Ok(item) => yield Ok(item),
                        Err(e) => yield Err(e),
                    }
                }
                chunk_start = newline_pos + 1;
            }

            let remainder = &chunk[chunk_start..];
            if buffer.len().saturating_add(remainder.len()) > MAX_SSE_LINE_BYTES {
                yield Err(sse_line_too_long_error());
                buffer.clear();
                break;
            }
            buffer.extend_from_slice(remainder);
        }

        if !buffer.is_empty()
            && let Ok(line) = std::str::from_utf8(&buffer)
                && let Some(result) = process_responses_line(line) {
                    match result {
                        Ok(item) => yield Ok(item),
                        Err(e) => yield Err(e),
                    }
                }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oversized_line_stream()
    -> impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static {
        futures_util::stream::iter([
            Ok(bytes::Bytes::from(vec![b'a'; MAX_SSE_LINE_BYTES])),
            Ok(bytes::Bytes::from_static(b"a")),
        ])
    }

    #[tokio::test]
    async fn chat_sse_rejects_oversized_line_without_newline() {
        let stream = parse_sse_stream(oversized_line_stream());
        futures_util::pin_mut!(stream);

        let error = stream
            .next()
            .await
            .expect("stream should report an error")
            .expect_err("oversized line should fail");

        assert!(matches!(
            error,
            Error::Stream(message) if message.contains("SSE line exceeds")
        ));
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn responses_sse_rejects_oversized_line_without_newline() {
        let stream = parse_responses_sse_stream(oversized_line_stream());
        futures_util::pin_mut!(stream);

        let error = stream
            .next()
            .await
            .expect("stream should report an error")
            .expect_err("oversized line should fail");

        assert!(matches!(
            error,
            Error::Stream(message) if message.contains("SSE line exceeds")
        ));
        assert!(stream.next().await.is_none());
    }
}
