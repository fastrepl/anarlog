mod openrouter;

pub use openrouter::OpenRouterProvider;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::types::ChatCompletionRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationMetadata {
    pub generation_id: String,
    pub model: Option<String>,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// Bounds the partial-frame buffer so a malformed stream without newlines
// cannot grow it indefinitely; analytics parsing is best-effort.
const MAX_PENDING_STREAM_BYTES: usize = 1024 * 1024;

pub struct StreamAccumulator {
    pub generation_id: Option<String>,
    pub model: Option<String>,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pending: Vec<u8>,
}

impl Default for StreamAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self {
            generation_id: None,
            model: None,
            input_tokens: 0,
            output_tokens: 0,
            pending: Vec::new(),
        }
    }

    pub(crate) fn buffer_bytes(&mut self, chunk: &[u8]) {
        self.pending.extend_from_slice(chunk);
    }

    // Splitting at b'\n' is UTF-8 safe: continuation bytes are >= 0x80, so a
    // newline byte can never be part of a multibyte character.
    pub(crate) fn next_complete_line(&mut self) -> Option<Vec<u8>> {
        let newline = self.pending.iter().position(|byte| *byte == b'\n')?;
        let mut line: Vec<u8> = self.pending.drain(..=newline).collect();
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        Some(line)
    }

    pub(crate) fn take_pending_line(&mut self) -> Vec<u8> {
        let mut line = std::mem::take(&mut self.pending);
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        line
    }

    pub(crate) fn drop_oversized_pending(&mut self) {
        if self.pending.len() > MAX_PENDING_STREAM_BYTES {
            self.pending.clear();
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Failed to serialize request: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

pub trait Provider: Send + Sync {
    fn name(&self) -> &str;

    fn base_url(&self) -> &str;

    fn build_request(
        &self,
        request: &ChatCompletionRequest,
        models: Vec<String>,
        stream: bool,
    ) -> Result<serde_json::Value, ProviderError>;

    fn parse_response(&self, body: &[u8]) -> Result<GenerationMetadata, ProviderError>;

    fn parse_stream_chunk(&self, chunk: &[u8], accumulator: &mut StreamAccumulator);

    // Called once after the upstream stream ends so providers can parse data
    // buffered from a final frame that arrived without a trailing newline.
    fn finish_stream(&self, accumulator: &mut StreamAccumulator) {
        let _ = accumulator;
    }

    fn fetch_cost(
        &self,
        client: &Client,
        api_key: &str,
        generation_id: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<f64>> + Send + '_>> {
        let _ = (client, api_key, generation_id);
        Box::pin(async { None })
    }

    fn build_auth_header(&self, api_key: &str) -> String {
        format!("Bearer {}", api_key)
    }

    fn additional_headers(&self) -> Vec<(String, String)> {
        vec![]
    }
}
