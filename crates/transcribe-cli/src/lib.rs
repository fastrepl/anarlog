use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use clap::{Args, ValueEnum};
use colored::Colorize;
use futures_util::StreamExt;
use owhisper_client::{FinalizeHandle, ListenClient, ListenClientDual, RealtimeSttAdapter};
use owhisper_interface::MixedMessage;
use owhisper_interface::stream::StreamResponse;

use hypr_audio::{CaptureConfig, CaptureFrame};
use hypr_audio_utils::{chunk_size_for_stt, f32_to_i16_bytes};

pub use hypr_audio::AudioProvider;
pub use hypr_audio_actual::ActualAudio;

pub const DEFAULT_SAMPLE_RATE: u32 = 16_000;
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;

#[derive(Clone, Copy)]
pub enum ChannelKind {
    Mic,
    Speaker,
}

pub enum DisplayMode {
    Single(ChannelKind),
    Dual,
}

#[derive(Clone, ValueEnum)]
pub enum AudioSource {
    Input,
    Output,
    RawDual,
    AecDual,
}

impl AudioSource {
    pub fn is_dual(&self) -> bool {
        matches!(self, Self::RawDual | Self::AecDual)
    }

    fn uses_aec(&self) -> bool {
        matches!(self, Self::AecDual)
    }
}

#[derive(Args)]
pub struct AudioArgs {
    #[arg(long, default_value = "input")]
    pub audio: AudioSource,
}

pub fn create_single_audio_stream(
    audio: &Arc<dyn AudioProvider>,
    source: &AudioSource,
    sample_rate: u32,
) -> std::pin::Pin<
    Box<
        dyn futures_util::Stream<
                Item = MixedMessage<bytes::Bytes, owhisper_interface::ControlMessage>,
            > + Send,
    >,
> {
    let chunk_size = chunk_size_for_stt(sample_rate);
    match source {
        AudioSource::Input => {
            let capture = audio
                .open_mic_capture(None, sample_rate, chunk_size)
                .expect("failed to open mic capture");
            Box::pin(capture.map(|result| {
                let frame = result.unwrap_or_else(|e| panic!("capture failed: {e}"));
                MixedMessage::Audio(f32_to_i16_bytes(frame.raw_mic.iter().copied()))
            }))
        }
        AudioSource::Output => {
            let capture = audio
                .open_speaker_capture(sample_rate, chunk_size)
                .expect("failed to open speaker capture");
            Box::pin(capture.map(|result| {
                let frame = result.unwrap_or_else(|e| panic!("capture failed: {e}"));
                MixedMessage::Audio(f32_to_i16_bytes(frame.raw_speaker.iter().copied()))
            }))
        }
        AudioSource::RawDual | AudioSource::AecDual => {
            panic!("dual audio modes use create_dual_audio_stream")
        }
    }
}

pub fn create_dual_audio_stream(
    audio: &Arc<dyn AudioProvider>,
    source: &AudioSource,
    sample_rate: u32,
) -> std::pin::Pin<
    Box<
        dyn futures_util::Stream<
                Item = MixedMessage<
                    (bytes::Bytes, bytes::Bytes),
                    owhisper_interface::ControlMessage,
                >,
            > + Send,
    >,
> {
    let chunk_size = chunk_size_for_stt(sample_rate);
    let capture_stream = audio
        .open_capture(CaptureConfig {
            sample_rate,
            chunk_size,
            mic_device: None,
            enable_aec: source.uses_aec(),
        })
        .expect("failed to open realtime capture");
    let source = source.clone();

    Box::pin(capture_stream.map(move |result| {
        let frame = result.unwrap_or_else(|error| panic!("capture failed: {error}"));
        MixedMessage::Audio(capture_frame_to_bytes(&source, frame))
    }))
}

pub fn print_audio_info(audio: &dyn AudioProvider, source: &AudioSource, sample_rate: u32) {
    let source_name = match source {
        AudioSource::Input => "input",
        AudioSource::Output => "output",
        AudioSource::RawDual | AudioSource::AecDual => unreachable!(),
    };
    let chunk_size = chunk_size_for_stt(sample_rate);

    eprintln!("source: {} ({})", source_name, audio.default_device_name());
    eprintln!(
        "sample rate: {} Hz, chunk size: {} samples",
        sample_rate, chunk_size
    );
    eprintln!();
}

pub fn print_dual_audio_info(audio: &dyn AudioProvider, source: &AudioSource, sample_rate: u32) {
    let chunk_size = chunk_size_for_stt(sample_rate);
    let source_name = match source {
        AudioSource::RawDual => "raw-dual",
        AudioSource::AecDual => "aec-dual",
        AudioSource::Input | AudioSource::Output => unreachable!(),
    };

    eprintln!(
        "source: {} (input: {}, output: RealtimeSpeaker)",
        source_name,
        audio.default_device_name()
    );
    eprintln!(
        "sample rate: {} Hz, chunk size: {} samples, AEC: {}",
        sample_rate,
        chunk_size,
        if source.uses_aec() {
            "enabled"
        } else {
            "disabled"
        }
    );
    eprintln!();
}

pub fn default_listen_params() -> owhisper_interface::ListenParams {
    owhisper_interface::ListenParams {
        sample_rate: DEFAULT_SAMPLE_RATE,
        languages: vec![hypr_language::ISO639::En.into()],
        ..Default::default()
    }
}

pub async fn build_single_client<A: RealtimeSttAdapter>(
    api_base: impl Into<String>,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
) -> ListenClient<A> {
    let mut builder = ListenClient::builder()
        .adapter::<A>()
        .api_base(api_base.into())
        .params(params);

    if let Some(api_key) = api_key {
        builder = builder.api_key(api_key);
    }

    builder.build_single().await
}

pub async fn build_dual_client<A: RealtimeSttAdapter>(
    api_base: impl Into<String>,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
) -> ListenClientDual<A> {
    let mut builder = ListenClient::builder()
        .adapter::<A>()
        .api_base(api_base.into())
        .params(params);

    if let Some(api_key) = api_key {
        builder = builder.api_key(api_key);
    }

    builder.build_dual().await
}

pub async fn run_single_client<A: RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: AudioSource,
    client: ListenClient<A>,
    sample_rate: u32,
    timeout_secs: u64,
) {
    let kind = match source {
        AudioSource::Input => ChannelKind::Mic,
        AudioSource::Output => ChannelKind::Speaker,
        _ => unreachable!(),
    };

    print_audio_info(&*audio, &source, sample_rate);

    let audio_stream = create_single_audio_stream(&audio, &source, sample_rate);
    let (response_stream, handle) = client
        .from_realtime_audio(audio_stream)
        .await
        .expect("failed to connect");

    process_stream(
        response_stream,
        handle,
        timeout_secs,
        DisplayMode::Single(kind),
    )
    .await;
}

pub async fn run_dual_client<A: RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: AudioSource,
    client: ListenClientDual<A>,
    sample_rate: u32,
    timeout_secs: u64,
) {
    print_dual_audio_info(&*audio, &source, sample_rate);

    let audio_stream = create_dual_audio_stream(&audio, &source, sample_rate);
    let (response_stream, handle) = client
        .from_realtime_audio(audio_stream)
        .await
        .expect("failed to connect");

    process_stream(response_stream, handle, timeout_secs, DisplayMode::Dual).await;
}

pub struct LocalServer {
    addr: SocketAddr,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl LocalServer {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn api_base(&self, suffix: &str) -> String {
        format!("http://{}{}", self.addr, suffix)
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

pub async fn spawn_router(app: Router) -> LocalServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    LocalServer {
        addr,
        shutdown_tx: Some(shutdown_tx),
    }
}

pub async fn process_stream<S, H>(
    response_stream: S,
    handle: H,
    timeout_secs: u64,
    mode: DisplayMode,
) where
    S: futures_util::Stream<Item = Result<StreamResponse, owhisper_client::hypr_ws_client::Error>>,
    H: FinalizeHandle,
{
    futures_util::pin_mut!(response_stream);

    let t0 = Instant::now();
    let mut channels: Vec<(Transcript, Option<String>)> = match &mode {
        DisplayMode::Single(kind) => vec![(Transcript::new(t0, *kind), None)],
        DisplayMode::Dual => vec![
            (Transcript::new(t0, ChannelKind::Mic), None),
            (Transcript::new(t0, ChannelKind::Speaker), None),
        ],
    };

    let read_loop = async {
        while let Some(result) = response_stream.next().await {
            match result {
                Ok(StreamResponse::TranscriptResponse {
                    is_final,
                    channel,
                    channel_index,
                    ..
                }) => {
                    let text = channel
                        .alternatives
                        .first()
                        .map(|a| a.transcript.as_str())
                        .unwrap_or("");

                    let ch = match &mode {
                        DisplayMode::Single(_) => 0,
                        DisplayMode::Dual => {
                            channel_index.first().copied().unwrap_or(0).clamp(0, 1) as usize
                        }
                    };

                    let (transcript, last_confirmed) = &mut channels[ch];
                    if is_final {
                        if last_confirmed.as_deref() == Some(text) {
                            continue;
                        }
                        *last_confirmed = Some(text.to_string());
                        transcript.confirm(text);
                    } else {
                        transcript.set_partial(text);
                    }
                }
                Ok(StreamResponse::TerminalResponse { .. }) => break,
                Ok(StreamResponse::ErrorResponse { error_message, .. }) => {
                    eprintln!("\nerror: {}", error_message);
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("\nws error: {:?}", e);
                    break;
                }
            }
        }
    };

    let _ = tokio::time::timeout(Duration::from_secs(timeout_secs), read_loop).await;
    handle.finalize().await;
    eprintln!();
}

#[macro_export]
macro_rules! simple_provider_example {
    (
        adapter: $adapter:path,
        api_base: $api_base:expr,
        api_key_env: $api_key_env:literal,
        params: $params:expr $(,)?
    ) => {
        #[derive(::clap::Parser)]
        struct Args {
            #[command(flatten)]
            audio: $crate::AudioArgs,
        }

        #[::tokio::main]
        async fn main() {
            let args = <Args as ::clap::Parser>::parse();
            let audio: ::std::sync::Arc<dyn $crate::AudioProvider> =
                ::std::sync::Arc::new($crate::ActualAudio);

            if args.audio.audio.is_dual() {
                let client = $crate::build_dual_client::<$adapter>(
                    $api_base,
                    Some(::std::env::var($api_key_env).expect(concat!($api_key_env, " not set"))),
                    $params,
                )
                .await;

                $crate::run_dual_client(
                    audio,
                    args.audio.audio,
                    client,
                    $crate::DEFAULT_SAMPLE_RATE,
                    $crate::DEFAULT_TIMEOUT_SECS,
                )
                .await;
            } else {
                let client = $crate::build_single_client::<$adapter>(
                    $api_base,
                    Some(::std::env::var($api_key_env).expect(concat!($api_key_env, " not set"))),
                    $params,
                )
                .await;

                $crate::run_single_client(
                    audio,
                    args.audio.audio,
                    client,
                    $crate::DEFAULT_SAMPLE_RATE,
                    $crate::DEFAULT_TIMEOUT_SECS,
                )
                .await;
            }
        }
    };
}

fn capture_frame_to_bytes(
    source: &AudioSource,
    frame: CaptureFrame,
) -> (bytes::Bytes, bytes::Bytes) {
    let (mic, speaker) = match source {
        AudioSource::RawDual => frame.raw_dual(),
        AudioSource::AecDual => frame.aec_dual(),
        AudioSource::Input | AudioSource::Output => unreachable!(),
    };

    (
        f32_to_i16_bytes(mic.iter().copied()),
        f32_to_i16_bytes(speaker.iter().copied()),
    )
}

fn fmt_ts(secs: f64) -> String {
    let m = (secs / 60.0) as u32;
    let s = secs % 60.0;
    format!("{:02}:{:02}", m, s as u32)
}

struct Segment {
    time: f64,
    text: String,
}

struct Transcript {
    segments: Vec<Segment>,
    partial: String,
    t0: Instant,
    kind: ChannelKind,
}

impl Transcript {
    fn new(t0: Instant, kind: ChannelKind) -> Self {
        Self {
            segments: Vec::new(),
            partial: String::new(),
            t0,
            kind,
        }
    }

    fn elapsed(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    fn set_partial(&mut self, text: &str) {
        self.partial = text.to_string();
        self.render();
    }

    fn confirm(&mut self, text: &str) {
        self.segments.push(Segment {
            time: self.elapsed(),
            text: text.to_string(),
        });
        self.partial.clear();
        self.trim();
        self.render();
    }

    fn trim(&mut self) {
        const OVERHEAD: usize = 70;
        let max_chars = crossterm::terminal::size()
            .map(|(cols, _)| (cols as usize).saturating_sub(OVERHEAD))
            .unwrap_or(120);

        let partial_len = if self.partial.is_empty() {
            0
        } else {
            self.partial.len() + 1
        };
        let total_len: usize = self
            .segments
            .iter()
            .map(|s| s.text.len() + 1)
            .sum::<usize>()
            + partial_len;
        if total_len > max_chars {
            let drain_count = self.segments.len() * 2 / 3;
            if drain_count > 0 {
                self.segments.drain(..drain_count);
            }
        }
    }

    fn render(&self) {
        let confirmed: String = self
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        if confirmed.is_empty() && self.partial.is_empty() {
            return;
        }

        let to = self.elapsed();
        let from = self.segments.first().map(|s| fmt_ts(s.time));
        let prefix = format!("[{} / {}]", from.as_deref().unwrap_or("--:--"), fmt_ts(to)).dimmed();

        let colored_confirmed = match self.kind {
            ChannelKind::Mic => confirmed.truecolor(255, 190, 190).bold(),
            ChannelKind::Speaker => confirmed.truecolor(190, 200, 255).bold(),
        };

        let colored_partial = if self.partial.is_empty() {
            None
        } else {
            Some(match self.kind {
                ChannelKind::Mic => self.partial.truecolor(128, 95, 95),
                ChannelKind::Speaker => self.partial.truecolor(95, 100, 128),
            })
        };

        if let Some(partial) = colored_partial {
            eprintln!("{} {} {}", prefix, colored_confirmed, partial);
        } else {
            eprintln!("{} {}", prefix, colored_confirmed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_source_reports_dual_modes() {
        assert!(!AudioSource::Input.is_dual());
        assert!(!AudioSource::Output.is_dual());
        assert!(AudioSource::RawDual.is_dual());
        assert!(AudioSource::AecDual.is_dual());
    }

    #[test]
    fn capture_frame_to_bytes_preserves_channel_order() {
        let frame = CaptureFrame {
            raw_mic: std::sync::Arc::from([0.25_f32, -0.25]),
            raw_speaker: std::sync::Arc::from([0.75_f32, -0.75]),
            aec_mic: Some(std::sync::Arc::from([0.1_f32, -0.1])),
        };

        let (raw_mic, raw_speaker) = capture_frame_to_bytes(&AudioSource::RawDual, frame.clone());
        assert_eq!(&raw_mic[..], &[0x00, 0x20, 0x00, 0xe0]);
        assert_eq!(&raw_speaker[..], &[0x00, 0x60, 0x00, 0xa0]);

        let (aec_mic, aec_speaker) = capture_frame_to_bytes(&AudioSource::AecDual, frame);
        assert_eq!(&aec_mic[..], &[0xcc, 0x0c, 0x34, 0xf3]);
        assert_eq!(&aec_speaker[..], &[0x00, 0x60, 0x00, 0xa0]);
    }
}
