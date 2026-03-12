use std::path::PathBuf;
use std::time::{Duration, Instant};

use axum::Router;
use axum::error_handling::HandleError;
use axum::http::StatusCode;
use clap::{Parser, ValueEnum};
use colored::Colorize;
use futures_util::StreamExt;
use owhisper_client::{CactusAdapter, FinalizeHandle, ListenClient};
use owhisper_interface::MixedMessage;
use owhisper_interface::stream::StreamResponse;

use hypr_audio::AudioInput;
use hypr_audio_utils::{AudioFormatExt, chunk_size_for_stt};
use transcribe_cactus::TranscribeService;

#[derive(Clone, ValueEnum)]
enum AudioSource {
    Input,
    Output,
}

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "input")]
    audio: AudioSource,

    #[arg(long)]
    model: PathBuf,
}

fn fmt_ts(secs: f64) -> String {
    let m = (secs / 60.0) as u32;
    let s = secs % 60.0;
    format!("{:02}:{:04.1}", m, s)
}

struct Segment {
    time: f64,
    text: String,
}

struct Transcript {
    segments: Vec<Segment>,
    partial: String,
    t0: Instant,
}

impl Transcript {
    fn new(t0: Instant) -> Self {
        Self {
            segments: Vec::new(),
            partial: String::new(),
            t0,
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
        let total_len: usize = self.segments.iter().map(|s| s.text.len() + 1).sum();
        if total_len > 180 {
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

        let from = self.segments.first().map(|s| s.time).unwrap_or(0.0);
        let to = self.elapsed();
        let prefix = format!("[{} / {}]", fmt_ts(from), fmt_ts(to)).dimmed();

        if self.partial.is_empty() {
            eprintln!("{} {}", prefix, confirmed.bold().white());
        } else {
            eprintln!(
                "{} {} {}",
                prefix,
                confirmed.bold().white(),
                self.partial.dimmed()
            );
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    assert!(
        args.model.exists(),
        "model not found: {}",
        args.model.display()
    );

    let app = Router::new().route_service(
        "/v1/listen",
        HandleError::new(
            TranscribeService::builder().model_path(args.model).build(),
            |err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) },
        ),
    );

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

    let mut audio_input = match args.audio {
        AudioSource::Output => AudioInput::from_speaker(),
        AudioSource::Input => AudioInput::from_mic(None).expect("failed to open mic"),
    };

    let sample_rate: u32 = 16000;
    let chunk_size = chunk_size_for_stt(sample_rate);
    let source_name = match args.audio {
        AudioSource::Input => "input",
        AudioSource::Output => "output",
    };

    eprintln!("source: {} ({})", source_name, audio_input.device_name());
    eprintln!(
        "sample rate: {} Hz -> {} Hz, chunk size: {} samples",
        audio_input.sample_rate(),
        sample_rate,
        chunk_size
    );
    eprintln!("(set CACTUS_DEBUG=1 for raw engine output)");
    eprintln!();

    let api_base = format!("http://{}/v1", addr);
    let client = ListenClient::builder()
        .adapter::<CactusAdapter>()
        .api_base(&api_base)
        .params(owhisper_interface::ListenParams {
            sample_rate,
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_single()
        .await;

    let stream = audio_input.stream();
    let audio_stream = stream
        .to_i16_le_chunks(sample_rate, chunk_size)
        .map(|bytes| MixedMessage::Audio(bytes));

    let (response_stream, handle) = client
        .from_realtime_audio(Box::pin(audio_stream))
        .await
        .expect("failed to connect");
    futures_util::pin_mut!(response_stream);

    let mut transcript = Transcript::new(Instant::now());
    let mut last_confirmed: Option<String> = None;

    let read_loop = async {
        while let Some(result) = response_stream.next().await {
            match result {
                Ok(StreamResponse::TranscriptResponse {
                    is_final, channel, ..
                }) => {
                    let text = channel
                        .alternatives
                        .first()
                        .map(|a| a.transcript.as_str())
                        .unwrap_or("");

                    if is_final {
                        if last_confirmed.as_deref() == Some(text) {
                            continue;
                        }
                        last_confirmed = Some(text.to_string());
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

    let _ = tokio::time::timeout(Duration::from_secs(600), read_loop).await;
    handle.finalize().await;

    eprintln!();

    let _ = shutdown_tx.send(());
}
