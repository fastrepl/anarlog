mod audio;
mod display;
mod server;
mod stream;

use std::path::PathBuf;
use std::sync::Arc;

use owhisper_client::{CactusAdapter, ListenClient};

use hypr_audio::AudioProvider;
use hypr_audio_actual::ActualAudio;
use hypr_audio_mock::MockAudio;

use display::{ChannelKind, DisplayMode};

const SAMPLE_RATE: u32 = 16_000;
const TIMEOUT_SECS: u64 = 600;

#[derive(Clone, strum::EnumString, strum::Display)]
#[strum(serialize_all = "kebab-case")]
enum AudioSource {
    Input,
    Output,
    RawDual,
    AecDual,
    Mock,
}

impl AudioSource {
    fn is_dual(&self) -> bool {
        matches!(self, Self::RawDual | Self::AecDual | Self::Mock)
    }

    fn uses_aec(&self) -> bool {
        matches!(self, Self::AecDual)
    }
}

struct Args {
    audio: AudioSource,
    model: PathBuf,
}

impl Args {
    fn parse() -> Self {
        let mut args = pico_args::Arguments::from_env();

        let audio: AudioSource = args
            .opt_value_from_str("--audio")
            .unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            })
            .unwrap_or(AudioSource::Input);

        let model: PathBuf = args.value_from_str("--model").unwrap_or_else(|_| {
            eprintln!("error: --model <PATH> is required");
            std::process::exit(1);
        });

        Self { audio, model }
    }
}

/// Example:
/// cargo run -p transcribe-cactus --example live --features live-example -- --model ~/Library/Application\ Support/hyprnote/models/cactus/parakeet-tdt-0.6b-v3-int4 --audio mock
#[tokio::main]
async fn main() {
    let args = Args::parse();

    assert!(
        args.model.exists(),
        "model not found: {}",
        args.model.display()
    );

    let audio: Arc<dyn AudioProvider> = match args.audio {
        AudioSource::Mock => Arc::new(MockAudio::new(1)),
        _ => Arc::new(ActualAudio),
    };
    let server = server::spawn(args.model).await;
    let api_base = format!("http://{}/v1", server.addr);
    let params = owhisper_interface::ListenParams {
        sample_rate: SAMPLE_RATE,
        languages: vec![hypr_language::ISO639::En.into()],
        ..Default::default()
    };

    audio::print_info(&*audio, &args.audio);

    let make_builder = || {
        ListenClient::builder()
            .adapter::<CactusAdapter>()
            .api_base(&api_base)
            .params(params.clone())
    };

    if args.audio.is_dual() {
        let client = make_builder().build_dual().await;
        let audio_stream = audio::create_dual_stream(&audio, &args.audio);
        let (response_stream, handle) = client
            .from_realtime_audio(audio_stream)
            .await
            .expect("failed to connect");

        stream::process(response_stream, handle, DisplayMode::Dual).await;
    } else {
        let kind = match args.audio {
            AudioSource::Input => ChannelKind::Mic,
            AudioSource::Output => ChannelKind::Speaker,
            _ => unreachable!(),
        };

        let client = make_builder().build_single().await;
        let audio_stream = audio::create_single_stream(&audio, &args.audio);
        let (response_stream, handle) = client
            .from_realtime_audio(audio_stream)
            .await
            .expect("failed to connect");

        stream::process(response_stream, handle, DisplayMode::Single(kind)).await;
    }
}
