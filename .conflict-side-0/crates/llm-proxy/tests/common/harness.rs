#![allow(dead_code)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use llm_proxy::provider::OpenRouterProvider;
use llm_proxy::{GenerationEvent, LlmProxyConfig, MODEL_KEY_DEFAULT, StaticModelResolver};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::analytics::MockAnalytics;

impl MockAnalytics {
    pub async fn get_single_event(&self) -> GenerationEvent {
        let timeout = std::time::Duration::from_secs(10);
        let poll_interval = std::time::Duration::from_millis(50);
        let start = std::time::Instant::now();

        loop {
            let events = self.captured_events();
            if events.len() == 1 {
                return events.into_iter().next().unwrap();
            }
            if start.elapsed() > timeout {
                panic!(
                    "Timed out waiting for analytics event. Got {} events, expected 1",
                    events.len()
                );
            }
            tokio::time::sleep(poll_interval).await;
        }
    }
}

pub struct TestHarness {
    pub mock_server: MockServer,
    pub analytics: MockAnalytics,
}

impl TestHarness {
    pub async fn new() -> Self {
        Self {
            mock_server: MockServer::start().await,
            analytics: MockAnalytics::default(),
        }
    }

    pub fn config(&self) -> LlmProxyConfig {
        let resolver = StaticModelResolver::default()
            .with_models(MODEL_KEY_DEFAULT, vec!["openai/gpt-4.1-nano".into()]);
        LlmProxyConfig::new("test-api-key")
            .with_provider(Arc::new(OpenRouterProvider::new(self.mock_server.uri())))
            .with_model_resolver(Arc::new(resolver))
            .with_analytics(Arc::new(self.analytics.clone()))
    }

    pub fn config_no_analytics(&self) -> LlmProxyConfig {
        let resolver = StaticModelResolver::default()
            .with_models(MODEL_KEY_DEFAULT, vec!["openai/gpt-4.1-nano".into()]);
        LlmProxyConfig::new("test-api-key")
            .with_provider(Arc::new(OpenRouterProvider::new(self.mock_server.uri())))
            .with_model_resolver(Arc::new(resolver))
    }

    pub async fn mount_json_response(&self, response: serde_json::Value) {
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("Authorization", "Bearer test-api-key"))
            .and(header("Content-Type", "application/json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&response)
                    .insert_header("Content-Type", "application/json"),
            )
            .expect(1)
            .mount(&self.mock_server)
            .await;
    }

    pub async fn mount_stream_response<S: AsRef<str>>(&self, chunks: &[S]) {
        let stream_response = chunks
            .iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>()
            .join("\n\n");
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("Authorization", "Bearer test-api-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(&stream_response)
                    .insert_header("Content-Type", "text/event-stream"),
            )
            .expect(1)
            .mount(&self.mock_server)
            .await;
    }

    pub async fn mount_error_response(&self, status: u16, response: serde_json::Value) {
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("Authorization", "Bearer test-api-key"))
            .respond_with(
                ResponseTemplate::new(status)
                    .set_body_json(&response)
                    .insert_header("Content-Type", "application/json"),
            )
            .expect(1)
            .mount(&self.mock_server)
            .await;
    }
}

pub fn build_request(body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

pub fn stream_chunks(id: &str) -> [String; 4] {
    [
        format!(
            r#"data: {{"id":"{id}","model":"openai/gpt-4.1-nano","choices":[{{"index":0,"delta":{{"role":"assistant","content":""}},"finish_reason":null}}]}}"#
        ),
        format!(
            r#"data: {{"id":"{id}","model":"openai/gpt-4.1-nano","choices":[{{"index":0,"delta":{{"content":"hello"}},"finish_reason":null}}]}}"#
        ),
        format!(
            r#"data: {{"id":"{id}","model":"openai/gpt-4.1-nano","choices":[{{"index":0,"delta":{{}},"finish_reason":"stop"}}],"usage":{{"prompt_tokens":8,"completion_tokens":1}}}}"#
        ),
        "data: [DONE]".to_string(),
    ]
}

pub fn completion_response(id: &str, model: &str, content: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "model": model,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 1}
    })
}

pub async fn response_to_json(response: axum::http::Response<Body>) -> serde_json::Value {
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body_bytes).unwrap()
}

pub async fn response_to_string(response: axum::http::Response<Body>) -> String {
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8_lossy(&body_bytes).to_string()
}
