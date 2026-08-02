use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    Json, Router, body::Bytes, extract::RawQuery, http::StatusCode, response::IntoResponse,
    routing::post,
};
use owhisper_client::Provider;
use transcribe_proxy::{AnarlogRoutingConfig, SttProxyConfig};

use super::MockServerHandle;

pub struct MockBatchUpstream {
    pub addr: SocketAddr,
    queries: Arc<Mutex<Vec<String>>>,
}

impl MockBatchUpstream {
    pub fn first_query(&self) -> Option<String> {
        self.queries.lock().ok()?.first().cloned()
    }
}

pub async fn start_proxy(
    deepgram_upstream: Option<&str>,
    soniox_upstream: Option<&str>,
) -> SocketAddr {
    start_proxy_with(
        Provider::Deepgram,
        false,
        deepgram_upstream,
        soniox_upstream,
    )
    .await
}

pub async fn start_proxy_under_stt(
    default_provider: Provider,
    deepgram_upstream: Option<&str>,
    soniox_upstream: Option<&str>,
) -> SocketAddr {
    start_proxy_with(default_provider, true, deepgram_upstream, soniox_upstream).await
}

pub async fn start_mock_batch_upstream() -> MockBatchUpstream {
    let queries: Arc<Mutex<Vec<String>>> = Default::default();
    let captured_queries = queries.clone();

    let app = Router::new().route(
        "/v1/listen",
        post(move |query: RawQuery| {
            let captured_queries = captured_queries.clone();
            async move {
                if let Ok(mut queries) = captured_queries.lock() {
                    queries.push(query.0.unwrap_or_default());
                }

                Json(serde_json::json!({
                    "metadata": {},
                    "results": {
                        "channels": [{
                            "alternatives": [{
                                "transcript": "ok",
                                "confidence": 1.0,
                                "words": []
                            }]
                        }]
                    }
                }))
                .into_response()
            }
        }),
    );

    let addr = serve(app).await;
    MockBatchUpstream { addr, queries }
}

pub async fn start_mock_stereo_batch_upstream() -> MockBatchUpstream {
    let queries: Arc<Mutex<Vec<String>>> = Default::default();
    let captured_queries = queries.clone();

    let app = Router::new().route(
        "/v1/listen",
        post(move |query: RawQuery, body: Bytes| {
            let captured_queries = captured_queries.clone();
            async move {
                let query = query.0.unwrap_or_default();
                if let Ok(mut queries) = captured_queries.lock() {
                    queries.push(query.clone());
                }

                if !query.contains("multichannel=true") {
                    return (
                        StatusCode::BAD_REQUEST,
                        "stereo batch request did not enable multichannel transcription",
                    )
                        .into_response();
                }

                if let Err(error) = validate_remote_party_stereo_wav(&body) {
                    return (StatusCode::BAD_REQUEST, error).into_response();
                }

                Json(serde_json::json!({
                    "metadata": { "channels": 2 },
                    "results": {
                        "channels": [
                            {
                                "alternatives": [{
                                    "transcript": "",
                                    "confidence": 1.0,
                                    "words": []
                                }]
                            },
                            {
                                "alternatives": [{
                                    "transcript": "remote",
                                    "confidence": 1.0,
                                    "words": [{
                                        "word": "remote",
                                        "punctuated_word": "remote",
                                        "start": 0.0,
                                        "end": 0.5,
                                        "confidence": 1.0
                                    }]
                                }]
                            }
                        ]
                    }
                }))
                .into_response()
            }
        }),
    );

    let addr = serve(app).await;
    MockBatchUpstream { addr, queries }
}

fn validate_remote_party_stereo_wav(body: &[u8]) -> Result<(), String> {
    let mut reader = hound::WavReader::new(std::io::Cursor::new(body))
        .map_err(|error| format!("invalid WAV: {error}"))?;
    if reader.spec().channels != 2 {
        return Err("expected stereo WAV".to_string());
    }

    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid PCM samples: {error}"))?;
    let direct_mic_is_silent = samples.chunks_exact(2).all(|frame| frame[0] == 0);
    let remote_party_has_audio = samples.chunks_exact(2).any(|frame| frame[1] != 0);

    if !direct_mic_is_silent || !remote_party_has_audio {
        return Err(
            "expected silent direct-mic channel and active remote-party channel".to_string(),
        );
    }

    Ok(())
}

pub async fn wait_for_first_request(mock: &MockServerHandle, timeout: Duration) -> String {
    wait_for(timeout, || mock.captured_requests().first().cloned()).await
}

pub async fn wait_for_first_batch_query(batch: &MockBatchUpstream, timeout: Duration) -> String {
    wait_for(timeout, || batch.first_query()).await
}

pub async fn wait_for<T>(timeout: Duration, mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(value) = f() {
            return value;
        }

        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out within {timeout:?}"
        );

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn start_proxy_with(
    default_provider: Provider,
    mount_under_stt: bool,
    deepgram_upstream: Option<&str>,
    soniox_upstream: Option<&str>,
) -> SocketAddr {
    let mut env = transcribe_proxy::Env::default();
    if deepgram_upstream.is_some() {
        env.stt.deepgram_api_key = Some("test-key".to_string());
    }
    if soniox_upstream.is_some() {
        env.stt.soniox_api_key = Some("test-key".to_string());
    }

    let supabase_env = anlg_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    };

    let mut config = SttProxyConfig::new(&env, &supabase_env)
        .with_default_provider(default_provider)
        .with_anarlog_routing(AnarlogRoutingConfig::default());

    if let Some(url) = deepgram_upstream {
        config = config.with_upstream_url(Provider::Deepgram, url);
    }
    if let Some(url) = soniox_upstream {
        config = config.with_upstream_url(Provider::Soniox, url);
    }

    let app = if mount_under_stt {
        Router::new().nest("/stt", transcribe_proxy::router(config))
    } else {
        transcribe_proxy::router(config)
    };

    serve(app).await
}

async fn serve(app: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let client = reqwest::Client::new();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        match client.get(format!("http://{addr}/")).send().await {
            Ok(_) => {
                break;
            }
            Err(_) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "timed out waiting for test server to accept connections"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }

    addr
}
