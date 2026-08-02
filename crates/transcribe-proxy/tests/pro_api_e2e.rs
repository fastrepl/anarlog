mod common;

use std::time::Duration;

use common::{start_server, test_audio_stream_with_rate};
use futures_util::StreamExt;
use owhisper_client::{AnarlogAdapter, FinalizeHandle, ListenClient};
use owhisper_interface::ListenParams;
use owhisper_interface::stream::StreamResponse;
use transcribe_proxy::{AnarlogRoutingConfig, Env, SttProxyConfig};

fn required_key(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn pro_env() -> Env {
    let mut env = Env::default();
    env.stt.deepgram_api_key = Some(required_key("DEEPGRAM_API_KEY"));
    env.stt.assemblyai_api_key = Some(required_key("ASSEMBLYAI_API_KEY"));
    env.stt.soniox_api_key = Some(required_key("SONIOX_API_KEY"));
    env.stt.gladia_api_key = Some(required_key("GLADIA_API_KEY"));
    env.stt.elevenlabs_api_key = Some(required_key("ELEVENLABS_API_KEY"));
    env
}

fn supabase_env() -> anlg_api_env::SupabaseEnv {
    anlg_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    }
}

async fn assert_live_transcription(addr: std::net::SocketAddr) {
    let sample_rate = 16_000;
    let client = ListenClient::builder()
        .adapter::<AnarlogAdapter>()
        .api_base(format!("http://{addr}"))
        .params(ListenParams {
            model: Some("cloud".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            sample_rate,
            ..Default::default()
        })
        .build_single()
        .await
        .expect("Pro live client should be configured");

    let (stream, handle) = client
        .from_realtime_audio(test_audio_stream_with_rate(sample_rate))
        .await
        .expect("Pro live transcription should connect");
    futures_util::pin_mut!(stream);

    let saw_transcript = tokio::time::timeout(Duration::from_secs(45), async {
        while let Some(result) = stream.next().await {
            let response = result.expect("Pro live transcription should not fail");
            if let StreamResponse::TranscriptResponse { channel, .. } = response
                && channel
                    .alternatives
                    .first()
                    .is_some_and(|alternative| !alternative.transcript.trim().is_empty())
            {
                return true;
            }
        }
        false
    })
    .await
    .expect("Pro live transcription timed out");

    handle.finalize().await;
    assert!(saw_transcript, "Pro live transcription should return text");
}

fn remote_party_stereo_audio() -> Vec<u8> {
    let mut reader = hound::WavReader::open(anlg_data::english_1::AUDIO_PATH)
        .expect("test audio should be readable");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1, "test audio should be mono");
    assert_eq!(spec.sample_rate, 16_000, "test audio should be 16 kHz");
    assert_eq!(spec.bits_per_sample, 16, "test audio should be 16-bit");
    assert_eq!(
        spec.sample_format,
        hound::SampleFormat::Int,
        "test audio should use integer PCM",
    );

    let mut output = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(
        &mut output,
        hound::WavSpec {
            channels: 2,
            ..spec
        },
    )
    .expect("stereo test audio should be writable");
    for sample in reader.samples::<i16>() {
        writer
            .write_sample(0i16)
            .expect("direct-mic silence should be writable");
        writer
            .write_sample(sample.expect("test audio samples should be readable"))
            .expect("remote-party audio should be writable");
    }
    writer
        .finalize()
        .expect("stereo test audio should finalize");

    output.into_inner()
}

async fn assert_batch_transcription(addr: std::net::SocketAddr) {
    let audio = remote_party_stereo_audio();
    let response = reqwest::Client::new()
        .post(format!(
            "http://{addr}/listen?provider=anarlog&model=cloud&language=en&channels=2&sample_rate=16000"
        ))
        .header("content-type", "audio/wav")
        .body(audio)
        .send()
        .await
        .expect("Pro batch transcription request should complete");

    let status = response.status();
    let body = response
        .text()
        .await
        .expect("Pro batch transcription response should be readable");
    assert!(
        status.is_success(),
        "Pro batch transcription failed with {status}: {body}"
    );

    let response: owhisper_interface::batch::Response =
        serde_json::from_str(&body).expect("Pro batch response should be valid");
    assert_eq!(
        response.results.channels.len(),
        2,
        "Pro batch transcription should preserve both input channels",
    );
    let direct_mic = response.results.channels[0]
        .alternatives
        .first()
        .expect("direct-mic channel should include an alternative");
    let remote_party = response.results.channels[1]
        .alternatives
        .first()
        .expect("remote-party channel should include an alternative");

    assert!(
        direct_mic.transcript.trim().is_empty() && direct_mic.words.is_empty(),
        "silent direct-mic channel should not contain transcript words",
    );
    assert!(
        !remote_party.transcript.trim().is_empty() && !remote_party.words.is_empty(),
        "remote-party channel should contain transcript words",
    );
    assert!(
        remote_party.words.iter().all(|word| word.channel == 1),
        "remote-party words should remain on channel 1",
    );
}

#[ignore]
#[tokio::test]
async fn pro_transcription_api_supports_live_and_batch() {
    let _ = tracing_subscriber::fmt::try_init();

    let env = pro_env();
    let config = SttProxyConfig::new(&env, &supabase_env())
        .with_anarlog_routing(AnarlogRoutingConfig::default());
    let addr = start_server(config).await;

    assert_live_transcription(addr).await;
    assert_batch_transcription(addr).await;
}
