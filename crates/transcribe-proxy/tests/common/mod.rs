#![allow(dead_code)]

pub mod fixtures;
pub mod mock_upstream;
pub mod recording;

#[allow(unused_imports)]
pub use fixtures::load_fixture;
#[allow(unused_imports)]
pub use mock_upstream::{MockServerHandle, MockUpstreamConfig, start_mock_server_with_config};
#[allow(unused_imports)]
pub use recording::{Direction, MessageKind, WsMessage, WsRecording};

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use owhisper_client::Provider;
use transcribe_proxy::{
    HyprnoteRoutingConfig, SttAnalyticsReporter, SttEvent, SttProxyConfig, router,
};

fn test_supabase_env() -> hypr_api_env::SupabaseEnv {
    hypr_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    }
}

#[derive(Default, Clone)]
pub struct MockAnalytics {
    pub events: Arc<Mutex<Vec<SttEvent>>>,
}

impl SttAnalyticsReporter for MockAnalytics {
    fn report_stt(
        &self,
        event: SttEvent,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        let events = self.events.clone();
        Box::pin(async move {
            events.lock().unwrap().push(event);
        })
    }
}

pub async fn start_server(config: SttProxyConfig) -> SocketAddr {
    let app = router(config);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    addr
}

pub async fn start_server_with_provider(provider: Provider, api_key: String) -> SocketAddr {
    let env = env_with_provider(provider, api_key);
    let config = SttProxyConfig::new(&env, &test_supabase_env())
        .with_default_provider(provider)
        .with_hyprnote_routing(HyprnoteRoutingConfig::default());
    start_server(config).await
}

pub async fn start_server_with_upstream_url(provider: Provider, upstream_url: &str) -> SocketAddr {
    let env = env_with_provider(provider, "mock-api-key".to_string());
    let config = SttProxyConfig::new(&env, &test_supabase_env())
        .with_default_provider(provider)
        .with_upstream_url(provider, upstream_url);
    start_server(config).await
}

pub fn env_with_provider(provider: Provider, api_key: String) -> transcribe_proxy::Env {
    let mut env = transcribe_proxy::Env::default();
    match provider {
        Provider::Deepgram => env.stt.deepgram_api_key = Some(api_key),
        Provider::AssemblyAI => env.stt.assemblyai_api_key = Some(api_key),
        Provider::Soniox => env.stt.soniox_api_key = Some(api_key),
        Provider::Fireworks => env.stt.fireworks_api_key = Some(api_key),
        Provider::OpenAI => env.stt.openai_api_key = Some(api_key),
        Provider::Gladia => env.stt.gladia_api_key = Some(api_key),
        Provider::ElevenLabs => env.stt.elevenlabs_api_key = Some(api_key),
        Provider::DashScope => env.stt.dashscope_api_key = Some(api_key),
        Provider::Mistral => env.stt.mistral_api_key = Some(api_key),
    }
    env
}

pub fn test_audio_stream() -> impl futures_util::Stream<
    Item = owhisper_interface::MixedMessage<bytes::Bytes, owhisper_interface::ControlMessage>,
> + Send
+ Unpin
+ 'static {
    test_audio_stream_with_rate(16000)
}

pub fn test_audio_stream_with_rate(
    sample_rate: u32,
) -> impl futures_util::Stream<
    Item = owhisper_interface::MixedMessage<bytes::Bytes, owhisper_interface::ControlMessage>,
> + Send
+ Unpin
+ 'static {
    use hypr_audio_utils::AudioFormatExt;

    // chunk_samples should be proportional to sample_rate to maintain 100ms chunks
    let chunk_samples = (sample_rate / 10) as usize;

    let audio = rodio::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
    ))
    .unwrap()
    .to_i16_le_chunks(sample_rate, chunk_samples);

    Box::pin(tokio_stream::StreamExt::throttle(
        audio.map(owhisper_interface::MixedMessage::Audio),
        Duration::from_millis(100),
    ))
}
