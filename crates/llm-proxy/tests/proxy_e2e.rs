mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use common::analytics::{MockAnalytics, simple_message, stream_request};
use llm_proxy::{LlmProxyConfig, MODEL_KEY_DEFAULT, StaticModelResolver, router};

async fn start_server(config: LlmProxyConfig) -> SocketAddr {
    let app = router(config);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    addr
}

fn real_config(analytics: MockAnalytics) -> LlmProxyConfig {
    let api_key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");
    let resolver = StaticModelResolver::default().with_models(
        MODEL_KEY_DEFAULT,
        vec![
            "moonshotai/kimi-k2-0905".into(),
            "anthropic/claude-haiku-4.5".into(),
        ],
    );
    LlmProxyConfig::new(api_key)
        .with_model_resolver(Arc::new(resolver))
        .with_analytics(Arc::new(analytics))
}

mod proxy_e2e {
    use super::*;

    pub mod openrouter {
        use super::*;

        pub mod non_streaming {
            use super::*;

            #[ignore]
            #[tokio::test]
            async fn test_proxy_non_streaming() {
                let _ = tracing_subscriber::fmt::try_init();

                let analytics = MockAnalytics::default();
                let addr = start_server(real_config(analytics.clone())).await;

                let client = reqwest::Client::new();
                let response = client
                    .post(format!("http://{}/chat/completions", addr))
                    .header("Content-Type", "application/json")
                    .json(&simple_message("Say 'hello' and nothing else."))
                    .send()
                    .await
                    .expect("failed to send request");

                assert_eq!(
                    response.status(),
                    StatusCode::OK,
                    "expected OK status, got {}",
                    response.status()
                );

                let body: serde_json::Value =
                    response.json().await.expect("failed to parse response");

                assert!(body.get("id").is_some(), "response should have an id");
                assert!(
                    body.get("choices").is_some(),
                    "response should have choices"
                );

                let content = body["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("");
                println!("[proxy:openrouter] non-streaming response: {}", content);

                tokio::time::sleep(Duration::from_secs(3)).await;

                let events = analytics.captured_events();
                assert!(!events.is_empty(), "expected at least one analytics event");

                let event = &events[0];
                assert!(
                    !event.generation_id.is_empty(),
                    "generation_id should not be empty"
                );
                assert!(!event.model.is_empty(), "model should not be empty");
                assert_eq!(event.http_status, 200, "http_status should be 200");
                assert!(event.input_tokens > 0, "input_tokens should be > 0");
                assert!(event.output_tokens > 0, "output_tokens should be > 0");
                assert!(event.latency > 0.0, "latency should be > 0");
            }
        }

        pub mod streaming {
            use super::*;

            #[ignore]
            #[tokio::test]
            async fn test_proxy_streaming() {
                let _ = tracing_subscriber::fmt::try_init();

                let analytics = MockAnalytics::default();
                let addr = start_server(real_config(analytics.clone())).await;

                let client = reqwest::Client::new();
                let response = client
                    .post(format!("http://{}/chat/completions", addr))
                    .header("Content-Type", "application/json")
                    .json(&stream_request("Say 'hello' and nothing else."))
                    .send()
                    .await
                    .expect("failed to send request");

                assert_eq!(
                    response.status(),
                    StatusCode::OK,
                    "expected OK status, got {}",
                    response.status()
                );

                let body_str = response.text().await.expect("failed to read response body");

                assert!(
                    body_str.contains("data: "),
                    "streaming response should contain 'data: '"
                );
                println!(
                    "[proxy:openrouter] streaming response length: {} bytes",
                    body_str.len()
                );

                tokio::time::sleep(Duration::from_secs(3)).await;

                let events = analytics.captured_events();
                assert!(!events.is_empty(), "expected at least one analytics event");

                let event = &events[0];
                assert!(
                    !event.generation_id.is_empty(),
                    "generation_id should not be empty"
                );
                assert!(!event.model.is_empty(), "model should not be empty");
                assert_eq!(event.http_status, 200, "http_status should be 200");
                assert!(event.latency > 0.0, "latency should be > 0");
            }
        }
    }
}
