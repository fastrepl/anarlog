use std::path::PathBuf;

use owhisper_client::{
    AssemblyAIAdapter, BatchClient, BatchSttAdapter, DeepgramAdapter, ElevenLabsAdapter,
    FireworksAdapter, GladiaAdapter, OpenAIAdapter, Provider, SonioxAdapter,
};
use owhisper_interface::ListenParams;

fn batch_transcript(response: &owhisper_interface::batch::Response) -> &str {
    response
        .results
        .channels
        .first()
        .and_then(|channel| channel.alternatives.first())
        .map(|alternative| alternative.transcript.as_str())
        .unwrap_or("")
}

async fn run_direct_batch_e2e<A: BatchSttAdapter>(provider: Provider) {
    let _ = tracing_subscriber::fmt::try_init();

    let api_key = std::env::var(provider.env_key_name())
        .unwrap_or_else(|_| panic!("{} must be set", provider.env_key_name()));
    let client = BatchClient::<A>::builder()
        .api_base(provider.default_api_base())
        .api_key(api_key)
        .params(ListenParams {
            model: Some(provider.default_batch_model().to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        })
        .build();

    let response = client
        .transcribe_file(PathBuf::from(hypr_data::english_1::AUDIO_PATH))
        .await
        .unwrap_or_else(|error| panic!("[{provider}] batch transcription failed: {error}"));
    let transcript = batch_transcript(&response);

    println!("[{provider}] batch transcript: {transcript}");

    assert!(
        !transcript.trim().is_empty(),
        "[{provider}] expected a non-empty transcript"
    );
}

macro_rules! direct_batch_test {
    ($name:ident, $adapter:ty, $provider:expr) => {
        #[ignore]
        #[tokio::test]
        async fn $name() {
            run_direct_batch_e2e::<$adapter>($provider).await;
        }
    };
}

mod direct_batch {
    use super::*;

    direct_batch_test!(deepgram, DeepgramAdapter, Provider::Deepgram);
    direct_batch_test!(assemblyai, AssemblyAIAdapter, Provider::AssemblyAI);
    direct_batch_test!(soniox, SonioxAdapter, Provider::Soniox);
    direct_batch_test!(gladia, GladiaAdapter, Provider::Gladia);
    direct_batch_test!(fireworks, FireworksAdapter, Provider::Fireworks);
    direct_batch_test!(openai, OpenAIAdapter, Provider::OpenAI);
    direct_batch_test!(elevenlabs, ElevenLabsAdapter, Provider::ElevenLabs);
}
