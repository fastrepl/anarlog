mod common;

use common::analytics::*;
use common::harness::*;

use std::sync::Arc;

use axum::http::StatusCode;
use llm_proxy::{LlmProxyConfig, MODEL_KEY_DEFAULT, StaticModelResolver, router};
use tower::ServiceExt;

mod basic {
    use super::*;

    #[tokio::test]
    async fn non_streaming() {
        let harness = TestHarness::new().await;
        harness
            .mount_json_response(completion_response(
                "gen-test-123",
                "openai/gpt-4.1-nano",
                "hello",
            ))
            .await;

        let response = router(harness.config())
            .oneshot(build_request(simple_message(
                "Say 'hello' and nothing else.",
            )))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response_to_json(response).await;
        assert_eq!(body["id"], "gen-test-123");
        assert_eq!(body["model"], "openai/gpt-4.1-nano");
        assert_eq!(body["choices"][0]["message"]["content"], "hello");

        let event = harness.analytics.get_single_event().await;
        assert_eq!(event.generation_id, "gen-test-123");
        assert_eq!(event.model, "openai/gpt-4.1-nano");
        assert_eq!(event.http_status, 200);
        assert_eq!(event.input_tokens, 10);
        assert_eq!(event.output_tokens, 1);
        assert!(event.latency > 0.0);
    }

    #[tokio::test]
    async fn streaming() {
        let harness = TestHarness::new().await;
        harness
            .mount_stream_response(&stream_chunks("gen-stream-456"))
            .await;

        let response = router(harness.config())
            .oneshot(build_request(stream_request(
                "Say 'hello' and nothing else.",
            )))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("Content-Type").unwrap(),
            "text/event-stream"
        );

        let body_str = response_to_string(response).await;
        assert!(body_str.contains("data: "));
        assert!(body_str.contains("gen-stream-456"));
        assert!(body_str.contains("[DONE]"));

        let event = harness.analytics.get_single_event().await;
        assert_eq!(event.generation_id, "gen-stream-456");
        assert_eq!(event.model, "openai/gpt-4.1-nano");
        assert_eq!(event.http_status, 200);
        assert_eq!(event.input_tokens, 8);
        assert_eq!(event.output_tokens, 1);
        assert!(event.latency > 0.0);
    }
}

mod features {
    use super::*;

    #[tokio::test]
    async fn request_transformation() {
        let harness = TestHarness::new().await;
        harness
            .mount_json_response(completion_response(
                "gen-transform-test",
                "openai/gpt-4.1-nano",
                "test",
            ))
            .await;

        let response = router(harness.config_no_analytics())
            .oneshot(build_request(serde_json::json!({
                "messages": [{"role": "user", "content": "test message"}],
                "temperature": 0.7,
                "max_tokens": 50
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response_to_json(response).await;
        assert_eq!(body["id"], "gen-transform-test");
        assert_eq!(body["model"], "openai/gpt-4.1-nano");
        assert_eq!(body["choices"][0]["message"]["content"], "test");
    }
}

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn upstream_error_propagates() {
        let harness = TestHarness::new().await;
        harness
            .mount_error_response(
                429,
                serde_json::json!({
                    "error": {
                        "message": "Rate limit exceeded",
                        "type": "rate_limit_error",
                        "code": "rate_limit_exceeded"
                    }
                }),
            )
            .await;

        let response = router(harness.config_no_analytics())
            .oneshot(build_request(simple_message("Hello")))
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 429);

        let body = response_to_json(response).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Rate limit")
        );
    }
}

mod e2e {
    use super::*;

    fn real_config(analytics: MockAnalytics) -> LlmProxyConfig {
        let api_key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");
        let resolver = StaticModelResolver::default()
            .with_models(MODEL_KEY_DEFAULT, vec!["openai/gpt-4.1-nano".into()]);
        LlmProxyConfig::new(api_key)
            .with_model_resolver(Arc::new(resolver))
            .with_analytics(Arc::new(analytics))
    }

    #[ignore]
    #[tokio::test]
    async fn non_streaming() {
        let analytics = MockAnalytics::default();
        let response = router(real_config(analytics.clone()))
            .oneshot(build_request(simple_message(
                "Say 'hello' and nothing else.",
            )))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response_to_json(response).await;
        assert!(body.get("id").is_some());
        assert!(body.get("choices").is_some());

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let event = &analytics.captured_events()[0];
        assert!(!event.generation_id.is_empty());
        assert!(!event.model.is_empty());
        assert_eq!(event.http_status, 200);
        assert!(event.input_tokens > 0);
        assert!(event.output_tokens > 0);
        assert!(event.latency > 0.0);
    }

    #[ignore]
    #[tokio::test]
    async fn streaming() {
        let analytics = MockAnalytics::default();
        let response = router(real_config(analytics.clone()))
            .oneshot(build_request(stream_request(
                "Say 'hello' and nothing else.",
            )))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_str = response_to_string(response).await;
        assert!(body_str.contains("data: "));

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let event = &analytics.captured_events()[0];
        assert!(!event.generation_id.is_empty());
        assert!(!event.model.is_empty());
        assert_eq!(event.http_status, 200);
        assert!(event.latency > 0.0);
    }
}
