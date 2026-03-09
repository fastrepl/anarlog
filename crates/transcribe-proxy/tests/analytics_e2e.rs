mod common;
use common::*;

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use owhisper_client::Provider;
use owhisper_client::{FinalizeHandle, ListenClient};
use owhisper_interface::stream::StreamResponse;
use transcribe_proxy::SttProxyConfig;

#[ignore]
#[tokio::test]
async fn e2e_deepgram_with_mock_analytics() {
    let _ = tracing_subscriber::fmt::try_init();

    let api_key = std::env::var("DEEPGRAM_API_KEY").expect("DEEPGRAM_API_KEY must be set");

    let mock_analytics = MockAnalytics::default();
    let events = mock_analytics.events.clone();

    let env = env_with_provider(Provider::Deepgram, api_key);
    let supabase = hypr_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    };
    let config = SttProxyConfig::new(&env, &supabase)
        .with_default_provider(Provider::Deepgram)
        .with_analytics(Arc::new(mock_analytics));

    let addr = start_server(config).await;

    let client = ListenClient::builder()
        .adapter::<owhisper_client::DeepgramAdapter>()
        .api_base(format!("http://{}", addr))
        .params(owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_single()
        .await;

    let input = test_audio_stream();
    let (stream, handle) = client.from_realtime_audio(input).await.unwrap();
    futures_util::pin_mut!(stream);

    let mut saw_transcript = false;
    let timeout = Duration::from_secs(30);

    let test_future = async {
        while let Some(result) = stream.next().await {
            match result {
                Ok(StreamResponse::TranscriptResponse { channel, .. }) => {
                    if let Some(alt) = channel.alternatives.first() {
                        if !alt.transcript.is_empty() {
                            println!("[analytics_test] {}", alt.transcript);
                            saw_transcript = true;
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    panic!("[analytics_test] error: {:?}", e);
                }
            }
        }
    };

    let _ = tokio::time::timeout(timeout, test_future).await;
    handle.finalize().await;

    tokio::time::sleep(Duration::from_secs(1)).await;

    assert!(saw_transcript, "expected at least one non-empty transcript");

    let captured_events = events.lock().unwrap();
    assert_eq!(captured_events.len(), 1);

    let event = &captured_events[0];
    assert_eq!(event.provider, "deepgram");

    let duration_secs = event.duration.as_secs_f64();
    assert!(
        duration_secs >= 5.0 && duration_secs <= 35.0,
        "expected duration between 5-35 seconds, got {:.2}s",
        duration_secs
    );
}
