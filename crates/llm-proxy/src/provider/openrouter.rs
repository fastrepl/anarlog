use anlg_openrouter::{
    Client as OpenRouterClient, Error as OpenRouterError, ProviderPreferences, ProviderSort,
    ProviderSortUnion,
};
use reqwest::Client;
use serde::Deserialize;

use crate::types::{ChatCompletionRequest, UsageInfo};

use super::{GenerationMetadata, Provider, ProviderError, StreamAccumulator};

pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

pub struct OpenRouterProvider {
    pub base_url: String,
}

impl Default for OpenRouterProvider {
    fn default() -> Self {
        Self {
            base_url: OPENROUTER_URL.to_string(),
        }
    }
}

impl OpenRouterProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    pub id: String,
    pub model: Option<String>,
    pub usage: Option<UsageInfo>,
}

fn parse_sse_data_line(line: &str, accumulator: &mut StreamAccumulator) {
    let Some(data) = line.strip_prefix("data: ") else {
        return;
    };

    if data.trim() == "[DONE]" {
        return;
    }

    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) else {
        return;
    };

    if accumulator.generation_id.is_none() {
        accumulator.generation_id = parsed.get("id").and_then(|v| v.as_str()).map(String::from);
    }

    if accumulator.model.is_none() {
        accumulator.model = parsed
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from);
    }

    if let Some(usage) = parsed
        .get("usage")
        .and_then(|u| serde_json::from_value::<UsageInfo>(u.clone()).ok())
    {
        accumulator.input_tokens = usage.input_tokens();
        accumulator.output_tokens = usage.output_tokens();
    }
}

impl Provider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn build_request(
        &self,
        request: &ChatCompletionRequest,
        models: Vec<String>,
        stream: bool,
    ) -> Result<serde_json::Value, ProviderError> {
        let mut body = serde_json::to_value(request)?;
        let obj = body.as_object_mut().ok_or_else(|| {
            ProviderError::InvalidRequest("chat completion request must be an object".to_string())
        })?;

        let provider_prefs = ProviderPreferences {
            sort: Some(ProviderSortUnion::Simple(ProviderSort::Latency)),
            preferred_min_throughput: None,
            ..Default::default()
        };

        obj.remove("model");
        obj.insert("models".to_string(), serde_json::to_value(models)?);
        obj.insert("stream".to_string(), serde_json::Value::Bool(stream));
        obj.insert(
            "provider".to_string(),
            serde_json::to_value(provider_prefs)?,
        );

        Ok(body)
    }

    fn parse_response(&self, body: &[u8]) -> Result<GenerationMetadata, ProviderError> {
        let parsed: OpenRouterResponse =
            serde_json::from_slice(body).map_err(|e| ProviderError::ParseError(e.to_string()))?;

        Ok(GenerationMetadata {
            generation_id: parsed.id,
            model: parsed.model,
            input_tokens: parsed.usage.as_ref().map(|u| u.input_tokens()).unwrap_or(0),
            output_tokens: parsed
                .usage
                .as_ref()
                .map(|u| u.output_tokens())
                .unwrap_or(0),
        })
    }

    fn parse_stream_chunk(&self, chunk: &[u8], accumulator: &mut StreamAccumulator) {
        accumulator.buffer_bytes(chunk);
        while let Some(line) = accumulator.next_complete_line() {
            if let Ok(line) = std::str::from_utf8(&line) {
                parse_sse_data_line(line, accumulator);
            }
        }
        accumulator.drop_oversized_pending();
    }

    fn finish_stream(&self, accumulator: &mut StreamAccumulator) {
        let line = accumulator.take_pending_line();
        if let Ok(line) = std::str::from_utf8(&line) {
            parse_sse_data_line(line, accumulator);
        }
    }

    fn fetch_cost(
        &self,
        client: &Client,
        api_key: &str,
        generation_id: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<f64>> + Send + '_>> {
        let client = client.clone();
        let api_key = api_key.to_string();
        let generation_id = generation_id.to_string();

        Box::pin(async move {
            let openrouter = OpenRouterClient::new(api_key).with_http_client(client);

            match openrouter
                .generation_total_cost_with_retry(&generation_id, 3)
                .await
            {
                Ok(cost) => cost,
                Err(OpenRouterError::Api { status, .. }) if status == 404 => {
                    tracing::debug!(
                        http.response.status_code = %status,
                        "generation_metadata_unavailable"
                    );
                    None
                }
                Err(OpenRouterError::Api { status, .. }) => {
                    tracing::warn!(
                        http.response.status_code = %status,
                        "generation_metadata_fetch_failed"
                    );
                    None
                }
                Err(_err) => {
                    tracing::warn!("generation_metadata_fetch_failed");
                    None
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChatMessage, Role};

    const STREAM_FIXTURE: &str = concat!(
        ": OPENROUTER PROCESSING\n\n",
        "data: {\"id\":\"gen-1\",\"model\":\"provider/モデル-1\"}\n\n",
        "data: {\"id\":\"gen-ignored\",\"choices\":[{\"delta\":{\"content\":\"héllo\"}}]}\n\n",
        "data: {\"id\":\"gen-1\",\"usage\":{\"prompt_tokens\":11,\"completion_tokens\":42}}\n\n",
        "data: [DONE]\n\n",
    );

    fn assert_fixture_result(accumulator: &StreamAccumulator) {
        assert_eq!(accumulator.generation_id.as_deref(), Some("gen-1"));
        assert_eq!(accumulator.model.as_deref(), Some("provider/モデル-1"));
        assert_eq!(accumulator.input_tokens, 11);
        assert_eq!(accumulator.output_tokens, 42);
    }

    #[test]
    fn stream_parsing_handles_whole_payload_in_one_chunk() {
        let provider = OpenRouterProvider::default();
        let mut accumulator = StreamAccumulator::new();
        provider.parse_stream_chunk(STREAM_FIXTURE.as_bytes(), &mut accumulator);
        provider.finish_stream(&mut accumulator);
        assert_fixture_result(&accumulator);
    }

    // Feeding one byte at a time exercises every possible chunk boundary,
    // including splits inside multibyte UTF-8 characters and JSON tokens.
    #[test]
    fn stream_parsing_is_invariant_to_chunk_boundaries() {
        let provider = OpenRouterProvider::default();
        let mut accumulator = StreamAccumulator::new();
        for byte in STREAM_FIXTURE.as_bytes() {
            provider.parse_stream_chunk(std::slice::from_ref(byte), &mut accumulator);
        }
        provider.finish_stream(&mut accumulator);
        assert_fixture_result(&accumulator);
    }

    #[test]
    fn stream_parsing_is_invariant_to_every_two_chunk_split() {
        let provider = OpenRouterProvider::default();
        let bytes = STREAM_FIXTURE.as_bytes();
        for split in 0..=bytes.len() {
            let mut accumulator = StreamAccumulator::new();
            provider.parse_stream_chunk(&bytes[..split], &mut accumulator);
            provider.parse_stream_chunk(&bytes[split..], &mut accumulator);
            provider.finish_stream(&mut accumulator);
            assert_fixture_result(&accumulator);
        }
    }

    #[test]
    fn stream_parsing_flushes_final_frame_without_trailing_newline() {
        let provider = OpenRouterProvider::default();
        let mut accumulator = StreamAccumulator::new();
        provider.parse_stream_chunk(
            b"data: {\"id\":\"gen-9\",\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":7}}",
            &mut accumulator,
        );
        assert!(accumulator.generation_id.is_none());
        provider.finish_stream(&mut accumulator);
        assert_eq!(accumulator.generation_id.as_deref(), Some("gen-9"));
        assert_eq!(accumulator.input_tokens, 3);
        assert_eq!(accumulator.output_tokens, 7);
    }

    #[test]
    fn stream_parsing_handles_crlf_lines_split_across_chunks() {
        let provider = OpenRouterProvider::default();
        let mut accumulator = StreamAccumulator::new();
        provider.parse_stream_chunk(b"data: {\"id\":\"gen-2\"}\r", &mut accumulator);
        provider.parse_stream_chunk(b"\ndata: [DONE]\r\n", &mut accumulator);
        provider.finish_stream(&mut accumulator);
        assert_eq!(accumulator.generation_id.as_deref(), Some("gen-2"));
    }

    #[test]
    fn build_request_replaces_model_with_openrouter_models() {
        let request = ChatCompletionRequest {
            model: Some("original-model".to_string()),
            messages: vec![ChatMessage {
                role: Role::User,
                content: Some(serde_json::Value::String("hello".to_string())),
                extra: serde_json::Map::new(),
            }],
            tools: None,
            tool_choice: None,
            stream: None,
            temperature: None,
            max_tokens: None,
            extra: serde_json::Map::new(),
        };

        let body = OpenRouterProvider::default()
            .build_request(
                &request,
                vec!["first-model".to_string(), "second-model".to_string()],
                true,
            )
            .unwrap();

        assert!(body.get("model").is_none());
        assert_eq!(
            body.get("models"),
            Some(&serde_json::json!(["first-model", "second-model"]))
        );
        assert_eq!(body.get("stream"), Some(&serde_json::Value::Bool(true)));
        assert!(body.get("provider").is_some());
    }
}
