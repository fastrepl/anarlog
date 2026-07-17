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

pub struct StreamAccumulator {
    pub generation_id: Option<String>,
    pub model: Option<String>,
    pub input_tokens: u32,
    pub output_tokens: u32,
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
