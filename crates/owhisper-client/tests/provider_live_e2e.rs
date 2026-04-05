use std::io::BufReader;
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use hypr_audio_utils::AudioFormatExt;
use owhisper_client::{
    AssemblyAIAdapter, DashScopeAdapter, DeepgramAdapter, ElevenLabsAdapter, FinalizeHandle,
    FireworksAdapter, GladiaAdapter, ListenClient, MistralAdapter, OpenAIAdapter, Provider,
    RealtimeSttAdapter, SonioxAdapter,
};
use owhisper_interface::{ControlMessage, MixedMessage, stream::StreamResponse};

fn timeout_secs() -> u64 {
    std::env::var("TEST_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
}

fn test_audio_stream_single()
-> impl Stream<Item = MixedMessage<bytes::Bytes, ControlMessage>> + Send + Unpin + 'static {
    let audio = rodio::Decoder::new(BufReader::new(
        std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
    ))
    .unwrap()
    .to_i16_le_chunks(16_000, 1_600);

    Box::pin(tokio_stream::StreamExt::throttle(
        audio.map(MixedMessage::Audio),
        Duration::from_millis(100),
    ))
}

fn test_audio_stream_dual()
-> impl Stream<Item = MixedMessage<(bytes::Bytes, bytes::Bytes), ControlMessage>>
+ Send
+ Unpin
+ 'static {
    let audio = rodio::Decoder::new(BufReader::new(
        std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
    ))
    .unwrap()
    .to_i16_le_chunks(16_000, 1_600);

    Box::pin(tokio_stream::StreamExt::throttle(
        audio.map(|chunk| MixedMessage::Audio((chunk.clone(), chunk))),
        Duration::from_millis(100),
    ))
}

async fn run_direct_live_single_e2e<A: RealtimeSttAdapter>(provider: Provider) {
    let _ = tracing_subscriber::fmt::try_init();

    let api_key = std::env::var(provider.env_key_name())
        .unwrap_or_else(|_| panic!("{} must be set", provider.env_key_name()));
    let client = ListenClient::builder()
        .adapter::<A>()
        .api_base(provider.default_api_base())
        .api_key(api_key)
        .params(owhisper_interface::ListenParams {
            model: Some(provider.default_live_model().to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_single()
        .await;

    let timeout = Duration::from_secs(timeout_secs());
    let input = test_audio_stream_single();
    let (stream, handle) = client.from_realtime_audio(input).await.unwrap();
    futures_util::pin_mut!(stream);

    let mut saw_transcript = false;

    let test_future = async {
        while let Some(result) = stream.next().await {
            match result {
                Ok(StreamResponse::TranscriptResponse { channel, .. }) => {
                    if let Some(alt) = channel.alternatives.first() {
                        if !alt.transcript.is_empty() {
                            println!("[{provider}] {}", alt.transcript);
                            saw_transcript = true;
                        }
                    }
                }
                Ok(_) => {}
                Err(error) => panic!("[{provider}] error: {error:?}"),
            }
        }
    };

    let _ = tokio::time::timeout(timeout, test_future).await;
    handle.finalize().await;

    assert!(
        saw_transcript,
        "[{provider}] expected at least one non-empty transcript"
    );
}

async fn run_direct_live_dual_e2e<A: RealtimeSttAdapter>(provider: Provider) {
    let _ = tracing_subscriber::fmt::try_init();

    let api_key = std::env::var(provider.env_key_name())
        .unwrap_or_else(|_| panic!("{} must be set", provider.env_key_name()));
    let client = ListenClient::builder()
        .adapter::<A>()
        .api_base(provider.default_api_base())
        .api_key(api_key)
        .params(owhisper_interface::ListenParams {
            model: Some(provider.default_live_model().to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_dual()
        .await;

    let timeout = Duration::from_secs(timeout_secs());
    let input = test_audio_stream_dual();
    let (stream, handle) = client.from_realtime_audio(input).await.unwrap();
    futures_util::pin_mut!(stream);

    let mut saw_transcript = false;

    let test_future = async {
        while let Some(result) = stream.next().await {
            match result {
                Ok(StreamResponse::TranscriptResponse {
                    channel,
                    channel_index,
                    ..
                }) => {
                    if let Some(alt) = channel.alternatives.first() {
                        if !alt.transcript.is_empty() {
                            println!(
                                "[{provider}] ch{}: {}",
                                channel_index.first().unwrap_or(&0),
                                alt.transcript
                            );
                            saw_transcript = true;
                        }
                    }
                }
                Ok(_) => {}
                Err(error) => panic!("[{provider}] error: {error:?}"),
            }
        }
    };

    let _ = tokio::time::timeout(timeout, test_future).await;
    handle.finalize().await;

    assert!(
        saw_transcript,
        "[{provider}] expected at least one non-empty transcript"
    );
}

macro_rules! direct_live_test {
    ($name:ident, $adapter:ty, $provider:expr) => {
        mod $name {
            use super::*;

            #[ignore]
            #[tokio::test]
            async fn single() {
                run_direct_live_single_e2e::<$adapter>($provider).await;
            }

            #[ignore]
            #[tokio::test]
            async fn dual() {
                run_direct_live_dual_e2e::<$adapter>($provider).await;
            }
        }
    };
}

direct_live_test!(deepgram, DeepgramAdapter, Provider::Deepgram);
direct_live_test!(assemblyai, AssemblyAIAdapter, Provider::AssemblyAI);
direct_live_test!(soniox, SonioxAdapter, Provider::Soniox);
direct_live_test!(gladia, GladiaAdapter, Provider::Gladia);
direct_live_test!(fireworks, FireworksAdapter, Provider::Fireworks);
direct_live_test!(elevenlabs, ElevenLabsAdapter, Provider::ElevenLabs);
direct_live_test!(dashscope, DashScopeAdapter, Provider::DashScope);
direct_live_test!(mistral, MistralAdapter, Provider::Mistral);
