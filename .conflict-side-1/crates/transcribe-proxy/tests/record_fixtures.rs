mod common;

use common::recording::{RecordingOptions, RecordingSession};
use common::*;

use futures_util::StreamExt;
use std::time::Duration;

use owhisper_client::Provider;
use owhisper_client::{FinalizeHandle, ListenClient, RealtimeSttAdapter};
use owhisper_interface::stream::StreamResponse;

async fn record_live_fixture<A: RealtimeSttAdapter>(
    provider: Provider,
    recording_opts: RecordingOptions,
    sample_rate: u32,
) {
    let _ = tracing_subscriber::fmt::try_init();

    let api_key = std::env::var(provider.env_key_name())
        .unwrap_or_else(|_| panic!("{} must be set", provider.env_key_name()));
    let addr = start_server_with_provider(provider, api_key).await;

    let recording_session = if recording_opts.enabled {
        Some(RecordingSession::new(provider))
    } else {
        None
    };

    let client = ListenClient::builder()
        .adapter::<A>()
        .api_base(format!("http://{}", addr))
        .params(owhisper_interface::ListenParams {
            model: Some(provider.default_live_model().to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            sample_rate,
            ..Default::default()
        })
        .build_single()
        .await;

    let provider_name = format!("record:{}", provider);
    let input = test_audio_stream_with_rate(sample_rate);
    let (stream, handle) = client.from_realtime_audio(input).await.unwrap();
    futures_util::pin_mut!(stream);

    let mut saw_transcript = false;
    let timeout = Duration::from_secs(30);

    let test_future = async {
        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    if let Some(ref session) = recording_session {
                        match serde_json::to_string(&response) {
                            Ok(json) => session.record_server_text(&json),
                            Err(e) => {
                                tracing::warn!("failed to serialize response for recording: {}", e)
                            }
                        }
                    }

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

    if let Some(session) = recording_session {
        if let Some(ref output_dir) = recording_opts.output_dir {
            std::fs::create_dir_all(output_dir).expect("failed to create fixtures directory");
            session
                .save_to_file(output_dir, &recording_opts.suffix)
                .expect("failed to save recording");
            println!("[{}] Recording saved to {:?}", provider_name, output_dir);
        }
    }

    assert!(
        saw_transcript,
        "[{}] expected at least one non-empty transcript",
        provider_name
    );
}

macro_rules! record_fixture_test {
    ($name:ident, $adapter:ty, $provider:expr) => {
        #[ignore]
        #[tokio::test]
        async fn $name() {
            let sample_rate = $provider.default_live_sample_rate();
            record_live_fixture::<$adapter>(
                $provider,
                RecordingOptions::from_env("normal"),
                sample_rate,
            )
            .await;
        }
    };
}

mod record {
    use super::*;

    record_fixture_test!(
        deepgram,
        owhisper_client::DeepgramAdapter,
        Provider::Deepgram
    );
    record_fixture_test!(
        assemblyai,
        owhisper_client::AssemblyAIAdapter,
        Provider::AssemblyAI
    );
    record_fixture_test!(soniox, owhisper_client::SonioxAdapter, Provider::Soniox);
    record_fixture_test!(gladia, owhisper_client::GladiaAdapter, Provider::Gladia);
    record_fixture_test!(
        fireworks,
        owhisper_client::FireworksAdapter,
        Provider::Fireworks
    );
    record_fixture_test!(
        elevenlabs,
        owhisper_client::ElevenLabsAdapter,
        Provider::ElevenLabs
    );
}
