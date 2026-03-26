mod common;

use common::*;

use futures_util::StreamExt;
use std::time::Duration;

use owhisper_client::Provider;
use owhisper_client::{FinalizeHandle, ListenClient, RealtimeSttAdapter};
use owhisper_interface::stream::StreamResponse;

async fn run_passthrough_live_test<A: RealtimeSttAdapter>(provider: Provider) {
    let _ = tracing_subscriber::fmt::try_init();

    let api_key = std::env::var(provider.env_key_name())
        .unwrap_or_else(|_| panic!("{} must be set", provider.env_key_name()));
    let addr = start_server_with_provider(provider, api_key).await;

    let sample_rate = provider.default_live_sample_rate();
    let params = owhisper_interface::ListenParams {
        model: Some(provider.default_live_model().to_string()),
        languages: vec![hypr_language::ISO639::En.into()],
        sample_rate,
        ..Default::default()
    };

    let provider_name = format!("passthrough:{}", provider);

    let client = ListenClient::builder()
        .adapter::<A>()
        .api_base(format!("http://{}", addr))
        .params(params)
        .build_single()
        .await;

    run_live_stream_test(client, provider_name, sample_rate).await;
}

async fn run_hyprnote_live_test(provider: Provider) {
    let _ = tracing_subscriber::fmt::try_init();

    let api_key = std::env::var(provider.env_key_name())
        .unwrap_or_else(|_| panic!("{} must be set", provider.env_key_name()));
    let addr = start_server_with_provider(provider, api_key).await;

    let sample_rate = provider.default_live_sample_rate();
    let params = owhisper_interface::ListenParams {
        model: Some(provider.default_live_model().to_string()),
        languages: vec![hypr_language::ISO639::En.into()],
        sample_rate,
        custom_query: Some(
            [("provider".to_string(), "hyprnote".to_string())]
                .into_iter()
                .collect(),
        ),
        ..Default::default()
    };

    let provider_name = format!("hyprnote:{}", provider);

    let client = ListenClient::builder()
        .adapter::<owhisper_client::HyprnoteAdapter>()
        .api_base(format!("http://{}", addr))
        .params(params)
        .build_single()
        .await;

    run_live_stream_test(client, provider_name, sample_rate).await;
}

async fn run_live_stream_test<A: RealtimeSttAdapter>(
    client: ListenClient<A>,
    provider_name: String,
    sample_rate: u32,
) {
    let input = test_audio_stream_with_rate(sample_rate);
    let (stream, handle) = client.from_realtime_audio(input).await.unwrap();
    futures_util::pin_mut!(stream);

    let mut saw_transcript = false;
    let timeout = Duration::from_secs(30);

    let test_future = async {
        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    if let StreamResponse::TranscriptResponse { channel, .. } = &response {
                        if let Some(alt) = channel.alternatives.first() {
                            if !alt.transcript.is_empty() {
                                println!("[{}] {}", provider_name, alt.transcript);
                                saw_transcript = true;
                            }
                        }
                    }
                }
                Err(e) => {
                    panic!("[{}] error: {:?}", provider_name, e);
                }
            }
        }
    };

    let _ = tokio::time::timeout(timeout, test_future).await;
    handle.finalize().await;

    assert!(
        saw_transcript,
        "[{}] expected at least one non-empty transcript",
        provider_name
    );
}

async fn run_passthrough_batch_test(provider: Provider) {
    let _ = tracing_subscriber::fmt::try_init();

    let api_key = std::env::var(provider.env_key_name())
        .unwrap_or_else(|_| panic!("{} must be set", provider.env_key_name()));
    let addr = start_server_with_provider(provider, api_key).await;

    let audio_bytes =
        std::fs::read(hypr_data::english_1::AUDIO_PATH).expect("failed to read test audio file");

    let model = provider.default_batch_model();
    let url = format!(
        "http://{}/listen?provider={}&model={}",
        addr, provider, model
    );

    run_batch_request(url, audio_bytes, format!("passthrough:{}", provider)).await;
}

async fn run_hyprnote_batch_test(provider: Provider) {
    let _ = tracing_subscriber::fmt::try_init();

    let api_key = std::env::var(provider.env_key_name())
        .unwrap_or_else(|_| panic!("{} must be set", provider.env_key_name()));
    let addr = start_server_with_provider(provider, api_key).await;

    let audio_bytes =
        std::fs::read(hypr_data::english_1::AUDIO_PATH).expect("failed to read test audio file");

    let model = provider.default_batch_model();
    let url = format!(
        "http://{}/listen?provider=hyprnote&model={}&language=en",
        addr, model
    );

    run_batch_request(url, audio_bytes, format!("hyprnote:{}", provider)).await;
}

async fn run_batch_request(url: String, audio_bytes: Vec<u8>, provider_name: String) {
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "audio/wav")
        .body(audio_bytes)
        .send()
        .await
        .expect("failed to send batch request");

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        panic!(
            "[{}] batch request failed with status: {}, body: {}",
            provider_name, status, body
        );
    }

    let batch_response: owhisper_interface::batch::Response = response
        .json()
        .await
        .expect("failed to parse batch response");

    let transcript = batch_response
        .results
        .channels
        .first()
        .and_then(|c| c.alternatives.first())
        .map(|a| a.transcript.as_str())
        .unwrap_or("");

    println!("[{}] batch transcript: {}", provider_name, transcript);

    assert!(
        !transcript.is_empty(),
        "[{}] expected non-empty transcript from batch transcription",
        provider_name
    );
}

macro_rules! passthrough_live_test {
    ($name:ident, $adapter:ty, $provider:expr) => {
        #[ignore]
        #[tokio::test]
        async fn $name() {
            run_passthrough_live_test::<$adapter>($provider).await;
        }
    };
}

macro_rules! hyprnote_live_test {
    ($name:ident, $provider:expr) => {
        #[ignore]
        #[tokio::test]
        async fn $name() {
            run_hyprnote_live_test($provider).await;
        }
    };
}

macro_rules! passthrough_batch_test {
    ($name:ident, $provider:expr) => {
        #[ignore]
        #[tokio::test]
        async fn $name() {
            run_passthrough_batch_test($provider).await;
        }
    };
}

macro_rules! hyprnote_batch_test {
    ($name:ident, $provider:expr) => {
        #[ignore]
        #[tokio::test]
        async fn $name() {
            run_hyprnote_batch_test($provider).await;
        }
    };
}

mod passthrough {
    use super::*;

    pub mod live {
        use super::*;

        passthrough_live_test!(
            deepgram,
            owhisper_client::DeepgramAdapter,
            Provider::Deepgram
        );
        passthrough_live_test!(
            assemblyai,
            owhisper_client::AssemblyAIAdapter,
            Provider::AssemblyAI
        );
        passthrough_live_test!(soniox, owhisper_client::SonioxAdapter, Provider::Soniox);
        passthrough_live_test!(gladia, owhisper_client::GladiaAdapter, Provider::Gladia);
        passthrough_live_test!(
            fireworks,
            owhisper_client::FireworksAdapter,
            Provider::Fireworks
        );
        passthrough_live_test!(openai, owhisper_client::OpenAIAdapter, Provider::OpenAI);
        passthrough_live_test!(
            elevenlabs,
            owhisper_client::ElevenLabsAdapter,
            Provider::ElevenLabs
        );
    }

    pub mod batch {
        use super::*;

        passthrough_batch_test!(deepgram, Provider::Deepgram);
        passthrough_batch_test!(assemblyai, Provider::AssemblyAI);
        passthrough_batch_test!(soniox, Provider::Soniox);
        passthrough_batch_test!(gladia, Provider::Gladia);
        passthrough_batch_test!(fireworks, Provider::Fireworks);
        passthrough_batch_test!(openai, Provider::OpenAI);
        passthrough_batch_test!(elevenlabs, Provider::ElevenLabs);
    }
}

mod hyprnote {
    use super::*;

    pub mod live {
        use super::*;

        hyprnote_live_test!(deepgram, Provider::Deepgram);
        hyprnote_live_test!(assemblyai, Provider::AssemblyAI);
        hyprnote_live_test!(soniox, Provider::Soniox);
        hyprnote_live_test!(gladia, Provider::Gladia);
        hyprnote_live_test!(fireworks, Provider::Fireworks);
        hyprnote_live_test!(openai, Provider::OpenAI);
        hyprnote_live_test!(elevenlabs, Provider::ElevenLabs);
    }

    pub mod batch {
        use super::*;

        hyprnote_batch_test!(deepgram, Provider::Deepgram);
        hyprnote_batch_test!(assemblyai, Provider::AssemblyAI);
        hyprnote_batch_test!(soniox, Provider::Soniox);
        hyprnote_batch_test!(gladia, Provider::Gladia);
        hyprnote_batch_test!(fireworks, Provider::Fireworks);
        hyprnote_batch_test!(openai, Provider::OpenAI);
        hyprnote_batch_test!(elevenlabs, Provider::ElevenLabs);
    }
}
