use std::time::Duration;

use futures_util::{Stream, StreamExt};
use hypr_audio_utils::AudioFormatExt;
use owhisper_interface::MixedMessage;
use owhisper_interface::stream::StreamResponse;

use crate::live::{FinalizeHandle, ListenClientDualInput, ListenClientInput};
use crate::{ListenClient, ListenClientDual, RealtimeSttAdapter};

#[macro_export]
macro_rules! define_realtime_e2e_tests {
    (
        adapter = $adapter:ty,
        provider = $provider:expr,
        env_key = $env_key:expr,
        base = $base:expr
    ) => {
        #[tokio::test]
        #[ignore]
        async fn test_build_single() {
            let client = $crate::ListenClient::builder()
                .adapter::<$adapter>()
                .api_base($base)
                .api_key(std::env::var($env_key).expect(concat!($env_key, " not set")))
                .params(owhisper_interface::ListenParams::default())
                .build_single();

            $crate::test_utils::run_single_test(client, $provider).await;
        }

        #[tokio::test]
        #[ignore]
        async fn test_build_dual() {
            let client = $crate::ListenClient::builder()
                .adapter::<$adapter>()
                .api_base($base)
                .api_key(std::env::var($env_key).expect(concat!($env_key, " not set")))
                .params(owhisper_interface::ListenParams::default())
                .build_dual();

            $crate::test_utils::run_dual_test(client, $provider).await;
        }
    };
    (
        adapter = $adapter:ty,
        provider = $provider:expr,
        env_key = $env_key:expr,
        base = $base:expr,
        params = $params:expr
    ) => {
        #[tokio::test]
        #[ignore]
        async fn test_build_single() {
            let client = $crate::ListenClient::builder()
                .adapter::<$adapter>()
                .api_base($base)
                .api_key(std::env::var($env_key).expect(concat!($env_key, " not set")))
                .params($params)
                .build_single();

            $crate::test_utils::run_single_test(client, $provider).await;
        }

        #[tokio::test]
        #[ignore]
        async fn test_build_dual() {
            let client = $crate::ListenClient::builder()
                .adapter::<$adapter>()
                .api_base($base)
                .api_key(std::env::var($env_key).expect(concat!($env_key, " not set")))
                .params($params)
                .build_dual();

            $crate::test_utils::run_dual_test(client, $provider).await;
        }
    };
}

fn timeout_secs() -> u64 {
    std::env::var("TEST_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
}

fn throttle_ms() -> u64 {
    100
}

fn chunk_samples() -> usize {
    1600
}

fn default_sample_rate() -> u32 {
    16000
}

pub fn test_audio_stream_single_with_rate(
    sample_rate: u32,
) -> impl Stream<Item = ListenClientInput> + Send + Unpin + 'static {
    let audio = rodio::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
    ))
    .unwrap()
    .to_i16_le_chunks(sample_rate, chunk_samples());

    Box::pin(tokio_stream::StreamExt::throttle(
        audio.map(|chunk| MixedMessage::Audio(chunk)),
        Duration::from_millis(throttle_ms()),
    ))
}

pub fn test_audio_stream_dual_with_rate(
    sample_rate: u32,
) -> impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static {
    let audio = rodio::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
    ))
    .unwrap()
    .to_i16_le_chunks(sample_rate, chunk_samples());

    Box::pin(tokio_stream::StreamExt::throttle(
        audio.map(|chunk| MixedMessage::Audio((chunk.clone(), chunk))),
        Duration::from_millis(throttle_ms()),
    ))
}

pub async fn run_single_test<A: RealtimeSttAdapter>(client: ListenClient<A>, provider_name: &str) {
    run_single_test_with_rate(client, provider_name, default_sample_rate()).await;
}

pub async fn run_single_test_with_rate<A: RealtimeSttAdapter>(
    client: ListenClient<A>,
    provider_name: &str,
    sample_rate: u32,
) {
    let _ = tracing_subscriber::fmt::try_init();

    let timeout = Duration::from_secs(timeout_secs());
    let input = test_audio_stream_single_with_rate(sample_rate);
    let (stream, handle) = client.from_realtime_audio(input).await.unwrap();
    futures_util::pin_mut!(stream);

    let mut saw_transcript = false;

    let test_future = async {
        while let Some(result) = stream.next().await {
            match result {
                Ok(StreamResponse::TranscriptResponse { channel, .. }) => {
                    if let Some(alt) = channel.alternatives.first() {
                        if !alt.transcript.is_empty() {
                            println!("[{}] {}", provider_name, alt.transcript);
                            saw_transcript = true;
                        }
                    }
                }
                Ok(_) => {}
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

pub async fn run_dual_test<A: RealtimeSttAdapter>(
    client: ListenClientDual<A>,
    provider_name: &str,
) {
    run_dual_test_with_rate(client, provider_name, default_sample_rate()).await;
}

pub async fn run_dual_test_with_rate<A: RealtimeSttAdapter>(
    client: ListenClientDual<A>,
    provider_name: &str,
    sample_rate: u32,
) {
    let _ = tracing_subscriber::fmt::try_init();

    let timeout = Duration::from_secs(timeout_secs());
    let input = test_audio_stream_dual_with_rate(sample_rate);
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
                                "[{}] ch{}: {}",
                                provider_name,
                                channel_index.first().unwrap_or(&0),
                                alt.transcript
                            );
                            saw_transcript = true;
                        }
                    }
                }
                Ok(_) => {}
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

pub struct UrlTestCase {
    pub name: &'static str,
    pub model: Option<&'static str>,
    pub languages: &'static [hypr_language::ISO639],
    pub contains: &'static [&'static str],
    pub not_contains: &'static [&'static str],
}

pub fn run_url_test_cases<A: RealtimeSttAdapter>(
    adapter: &A,
    api_base: &str,
    cases: &[UrlTestCase],
) {
    for case in cases {
        let params = owhisper_interface::ListenParams {
            model: case.model.map(str::to_string),
            languages: case.languages.iter().map(|l| (*l).into()).collect(),
            ..Default::default()
        };

        let url = adapter.build_ws_url(api_base, &params, 1);
        let url_str = url.as_str();

        for expected in case.contains {
            assert!(
                url_str.contains(expected),
                "[{}] URL should contain '{}' but got: {}",
                case.name,
                expected,
                url_str
            );
        }
        for unexpected in case.not_contains {
            assert!(
                !url_str.contains(unexpected),
                "[{}] URL should NOT contain '{}' but got: {}",
                case.name,
                unexpected,
                url_str
            );
        }
    }
}
