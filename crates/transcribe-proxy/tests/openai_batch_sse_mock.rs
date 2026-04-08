mod common;

use std::net::SocketAddr;

use axum::{
    Router,
    body::Body,
    extract::RawQuery,
    http::StatusCode,
    http::header,
    response::{IntoResponse, Response},
    routing::post,
};
use common::{env_with_provider, start_server};
use owhisper_client::Provider;
use owhisper_interface::batch_sse::BatchSseMessage;
use transcribe_proxy::{HyprnoteRoutingConfig, SttProxyConfig};

#[tokio::test]
async fn hyprnote_batch_sse_streams_openai_events_with_cactus_contract() {
    let upstream_addr = start_openai_sse_upstream().await;

    let env = env_with_provider(Provider::OpenAI, "mock-api-key".to_string());
    let supabase_env = hypr_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    };

    let config = SttProxyConfig::new(&env, &supabase_env)
        .with_default_provider(Provider::OpenAI)
        .with_upstream_url(Provider::OpenAI, &format!("http://{upstream_addr}/v1"))
        .with_hyprnote_routing(HyprnoteRoutingConfig::default());
    let addr = start_server(config).await;

    let audio_bytes =
        std::fs::read(hypr_data::english_1::AUDIO_PATH).expect("failed to read test audio");

    let response = reqwest::Client::new()
        .post(format!(
            "http://{addr}/listen?provider=hyprnote&model=gpt-4o-transcribe&language=en"
        ))
        .header("content-type", "audio/wav")
        .header("accept", "text/event-stream")
        .body(audio_bytes)
        .send()
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("text/event-stream"))
    );

    let body = response.text().await.expect("failed to read SSE response");
    let events = parse_batch_sse_messages(&body);

    let progress = events
        .iter()
        .find_map(|event| match event {
            BatchSseMessage::Progress { progress } => Some(progress),
            _ => None,
        })
        .expect("expected progress event");
    assert!(progress.percentage > 0.0);
    assert_eq!(progress.partial_text.as_deref(), Some("hello "));

    let result = events
        .iter()
        .find_map(|event| match event {
            BatchSseMessage::Result { response } => Some(response),
            _ => None,
        })
        .expect("expected result event");

    assert_eq!(
        result.results.channels[0].alternatives[0].transcript,
        "hello world"
    );
    assert_eq!(result.metadata["usage"]["total_tokens"], 3);
}

#[tokio::test]
async fn hyprnote_batch_sse_returns_sse_for_non_streaming_first_provider() {
    let deepgram_addr = start_json_batch_upstream("deepgram ok").await;

    let mut env = transcribe_proxy::Env::default();
    env.stt.deepgram_api_key = Some("deepgram-key".to_string());
    env.stt.openai_api_key = Some("openai-key".to_string());

    let supabase_env = hypr_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    };

    let config = SttProxyConfig::new(&env, &supabase_env)
        .with_default_provider(Provider::Deepgram)
        .with_upstream_url(Provider::Deepgram, &format!("http://{deepgram_addr}/v1"))
        .with_hyprnote_routing(HyprnoteRoutingConfig::default());
    let addr = start_server(config).await;

    let audio_bytes =
        std::fs::read(hypr_data::english_1::AUDIO_PATH).expect("failed to read test audio");

    let response = reqwest::Client::new()
        .post(format!(
            "http://{addr}/listen?provider=hyprnote&language=en"
        ))
        .header("content-type", "audio/wav")
        .header("accept", "text/event-stream")
        .body(audio_bytes)
        .send()
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("text/event-stream"))
    );

    let body = response.text().await.expect("failed to read SSE response");
    let events = parse_batch_sse_messages(&body);
    let result = events
        .iter()
        .find_map(|event| match event {
            BatchSseMessage::Result { response } => Some(response),
            _ => None,
        })
        .expect("expected result event");

    assert_eq!(
        result.results.channels[0].alternatives[0].transcript,
        "deepgram ok"
    );
}

#[tokio::test]
async fn direct_batch_sse_returns_sse_for_non_streaming_provider() {
    let deepgram_addr = start_json_batch_upstream("deepgram direct").await;

    let env = env_with_provider(Provider::Deepgram, "deepgram-key".to_string());
    let supabase_env = hypr_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    };

    let config = SttProxyConfig::new(&env, &supabase_env)
        .with_default_provider(Provider::Deepgram)
        .with_upstream_url(Provider::Deepgram, &format!("http://{deepgram_addr}/v1"));
    let addr = start_server(config).await;

    let audio_bytes =
        std::fs::read(hypr_data::english_1::AUDIO_PATH).expect("failed to read test audio");

    let response = reqwest::Client::new()
        .post(format!(
            "http://{addr}/listen?provider=deepgram&language=en"
        ))
        .header("content-type", "audio/wav")
        .header("accept", "text/event-stream")
        .body(audio_bytes)
        .send()
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("text/event-stream"))
    );

    let body = response.text().await.expect("failed to read SSE response");
    let events = parse_batch_sse_messages(&body);
    let result = events
        .iter()
        .find_map(|event| match event {
            BatchSseMessage::Result { response } => Some(response),
            _ => None,
        })
        .expect("expected result event");

    assert_eq!(
        result.results.channels[0].alternatives[0].transcript,
        "deepgram direct"
    );
}

#[tokio::test]
async fn hyprnote_batch_sse_falls_back_before_openai_emits_progress() {
    let openai_addr = start_openai_error_upstream(StatusCode::TOO_MANY_REQUESTS).await;
    let deepgram_addr = start_json_batch_upstream("deepgram fallback").await;

    let mut env = transcribe_proxy::Env::default();
    env.stt.deepgram_api_key = Some("deepgram-key".to_string());
    env.stt.openai_api_key = Some("openai-key".to_string());

    let supabase_env = hypr_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    };

    let config = SttProxyConfig::new(&env, &supabase_env)
        .with_default_provider(Provider::OpenAI)
        .with_upstream_url(Provider::OpenAI, &format!("http://{openai_addr}/v1"))
        .with_upstream_url(Provider::Deepgram, &format!("http://{deepgram_addr}/v1"))
        .with_hyprnote_routing(HyprnoteRoutingConfig {
            priorities: vec![Provider::OpenAI, Provider::Deepgram],
            retry_config: Default::default(),
        });
    let addr = start_server(config).await;

    let audio_bytes =
        std::fs::read(hypr_data::english_1::AUDIO_PATH).expect("failed to read test audio");

    let response = reqwest::Client::new()
        .post(format!(
            "http://{addr}/listen?provider=hyprnote&model=gpt-4o-transcribe&language=en"
        ))
        .header("content-type", "audio/wav")
        .header("accept", "text/event-stream")
        .body(audio_bytes)
        .send()
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.text().await.expect("failed to read SSE response");
    let events = parse_batch_sse_messages(&body);
    let result = events
        .iter()
        .find_map(|event| match event {
            BatchSseMessage::Result { response } => Some(response),
            _ => None,
        })
        .expect("expected result event");

    assert_eq!(
        result.results.channels[0].alternatives[0].transcript,
        "deepgram fallback"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, BatchSseMessage::Progress { .. })),
        "should not emit OpenAI progress before fallback"
    );
}

async fn start_openai_sse_upstream() -> SocketAddr {
    let app = Router::new().route(
        "/v1/audio/transcriptions",
        post(|| async { openai_sse_response() }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream listener");
    let addr = listener.local_addr().expect("upstream local addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve upstream");
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    addr
}

async fn start_openai_error_upstream(status: StatusCode) -> SocketAddr {
    let app = Router::new().route(
        "/v1/audio/transcriptions",
        post(move || async move {
            (
                status,
                [(header::CONTENT_TYPE, "application/json")],
                Body::from(r#"{"error":"rate_limited"}"#),
            )
                .into_response()
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream listener");
    let addr = listener.local_addr().expect("upstream local addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve upstream");
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    addr
}

async fn start_json_batch_upstream(transcript: &'static str) -> SocketAddr {
    let app = Router::new().route(
        "/v1/listen",
        post(move |_query: RawQuery| async move {
            (
                [(header::CONTENT_TYPE, "application/json")],
                Body::from(
                    serde_json::json!({
                        "metadata": {},
                        "results": {
                            "channels": [{
                                "alternatives": [{
                                    "transcript": transcript,
                                    "confidence": 1.0,
                                    "words": []
                                }]
                            }]
                        }
                    })
                    .to_string(),
                ),
            )
                .into_response()
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream listener");
    let addr = listener.local_addr().expect("upstream local addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve upstream");
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    addr
}

fn openai_sse_response() -> Response {
    let delta = serde_json::json!({
        "type": "transcript.text.delta",
        "delta": "hello ",
    });
    let done = serde_json::json!({
        "type": "transcript.text.done",
        "text": "hello world",
        "usage": {
            "type": "tokens",
            "input_tokens": 1,
            "output_tokens": 2,
            "total_tokens": 3,
        },
    });

    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        Body::from(format!("data: {delta}\n\ndata: {done}\n\n")),
    )
        .into_response()
}

fn parse_batch_sse_messages(body: &str) -> Vec<BatchSseMessage> {
    body.split("\n\n")
        .filter_map(|block| {
            let mut data = String::new();

            for line in block.lines() {
                if let Some(rest) = line.strip_prefix("data:") {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(rest.trim());
                }
            }

            if data.is_empty() {
                return None;
            }

            serde_json::from_str(&data).ok()
        })
        .collect()
}
