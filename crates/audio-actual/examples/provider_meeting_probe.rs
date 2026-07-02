use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use audio_actual::AudioInput;
use bytes::Bytes;
use clap::{Parser, ValueEnum};
use futures_util::{Stream, StreamExt};
use hypr_audio::CaptureConfig;
use owhisper_client::{
    DeepgramAdapter, DualHandle, FinalizeHandle, ListenClient, ListenClientDualInput,
    LocalSoniqoLiveHandle, Provider, SonioxAdapter,
};
use owhisper_interface::MixedMessage;
use owhisper_interface::stream::StreamResponse;
use serde::Serialize;
use tokio_stream::wrappers::ReceiverStream;

const DEFAULT_DURATION_SECS: u64 = 600;
const DEFAULT_WINDOW_SECS: u64 = 1;
const DEFAULT_SAMPLE_RATE: u32 = 16_000;
const DEFAULT_PROVIDER_QUEUE: usize = 64;
const DEFAULT_COLLECTOR_TIMEOUT_SECS: u64 = 15;
const DEFAULT_SPEAKER_PREFLIGHT_SECS: u64 = 5;

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "parakeet,deepgram,soniox"
    )]
    providers: Vec<ProbeProvider>,
    #[arg(long, value_delimiter = ',', default_value = "0.25,0.5,1.0")]
    playback_volumes: Vec<f32>,
    #[arg(long, default_value_t = DEFAULT_DURATION_SECS)]
    duration_secs: u64,
    #[arg(long, default_value_t = DEFAULT_SAMPLE_RATE)]
    sample_rate: u32,
    #[arg(long)]
    mic_device: Option<String>,
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long)]
    playback_file: Option<PathBuf>,
    #[arg(long)]
    no_aec: bool,
    #[arg(long)]
    allow_silent_speaker: bool,
    #[arg(long, default_value_t = DEFAULT_SPEAKER_PREFLIGHT_SECS)]
    speaker_preflight_secs: u64,
    #[arg(long, default_value_t = DEFAULT_WINDOW_SECS)]
    window_secs: u64,
    #[arg(long, default_value_t = -60.0)]
    speaker_active_dbfs: f32,
    #[arg(long)]
    deepgram_model: Option<String>,
    #[arg(long)]
    soniox_model: Option<String>,
    #[arg(long)]
    parakeet_model: Option<String>,
    #[arg(long)]
    deepgram_api_base: Option<String>,
    #[arg(long)]
    soniox_api_base: Option<String>,
    #[arg(long = "expected-phrase")]
    expected_phrases: Vec<String>,
    #[arg(long, default_value_t = 1)]
    min_speaker_results: usize,
    #[arg(long, default_value_t = 1)]
    min_speaker_phrase_hits: usize,
    #[arg(long, default_value_t = 0)]
    max_mic_results: usize,
    #[arg(long, default_value_t = 0)]
    max_mic_phrase_hits: usize,
    #[arg(long, default_value_t = 0)]
    max_cross_channel_duplicates: usize,
    #[arg(long)]
    allow_failures: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum, Serialize)]
enum ProbeProvider {
    Parakeet,
    Deepgram,
    Soniox,
}

impl ProbeProvider {
    fn as_str(self) -> &'static str {
        match self {
            Self::Parakeet => "parakeet",
            Self::Deepgram => "deepgram",
            Self::Soniox => "soniox",
        }
    }
}

impl fmt::Display for ProbeProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Serialize)]
struct MatrixSummary {
    started_unix_secs: u64,
    duration_secs_requested: u64,
    sample_rate: u32,
    playback_file: String,
    expected_phrases: Vec<String>,
    results: Vec<ProviderRunSummary>,
}

#[derive(Serialize)]
struct ProviderRunSummary {
    provider: ProbeProvider,
    model: String,
    playback_volume: f32,
    status: RunStatus,
    skip_reason: Option<String>,
    error: Option<String>,
    out_dir: String,
    transcript_jsonl: Option<String>,
    metrics_jsonl: Option<String>,
    duration_secs_captured: f64,
    audio: Option<AudioSummary>,
    transcript: Option<TranscriptSummary>,
    verdict: Option<Verdict>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum RunStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Serialize)]
struct AudioSummary {
    window_count: usize,
    speaker_active_windows: usize,
    aec_available_windows: usize,
    avg_mic_dbfs: Option<f32>,
    avg_speaker_dbfs: Option<f32>,
    max_mic_dbfs: Option<f32>,
    max_speaker_dbfs: Option<f32>,
}

#[derive(Serialize)]
struct WindowMetrics {
    window_index: usize,
    start_secs: f64,
    end_secs: f64,
    aec_available: bool,
    mic: LevelMetrics,
    speaker: LevelMetrics,
}

#[derive(Serialize)]
struct LevelMetrics {
    rms: f32,
    dbfs: Option<f32>,
    peak: f32,
}

#[derive(Serialize, Default, Clone)]
struct TranscriptSummary {
    total_results: usize,
    final_results: usize,
    mic_results: usize,
    speaker_results: usize,
    mic_final_results: usize,
    speaker_final_results: usize,
    mic_word_count: usize,
    speaker_word_count: usize,
    mic_expected_phrase_hits: usize,
    speaker_expected_phrase_hits: usize,
    near_duplicate_cross_channel_results: usize,
    mic_text_preview: String,
    speaker_text_preview: String,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct TranscriptRecord<'a> {
    provider: ProbeProvider,
    received_unix_millis: u64,
    response: &'a StreamResponse,
}

#[derive(Debug, Clone)]
struct TranscriptEvent {
    start: f64,
    channel: Option<i32>,
    is_final: bool,
    text: String,
}

#[derive(Serialize)]
struct Verdict {
    passed: bool,
    reasons: Vec<String>,
}

struct ProviderRunner {
    tx: tokio::sync::mpsc::Sender<ListenClientDualInput>,
    finalize: ProbeFinalizeHandle,
    collector: tokio::task::JoinHandle<anyhow::Result<TranscriptSummary>>,
}

enum ProbeFinalizeHandle {
    Cloud(DualHandle),
    Parakeet(LocalSoniqoLiveHandle),
}

impl ProbeFinalizeHandle {
    async fn finalize(&self) {
        match self {
            Self::Cloud(handle) => handle.finalize().await,
            Self::Parakeet(handle) => handle.finalize().await,
        }
    }
}

#[derive(Default)]
struct AudioAccumulator {
    window_mic: Vec<f32>,
    window_speaker: Vec<f32>,
    captured_samples: usize,
    window_count: usize,
    speaker_active_windows: usize,
    aec_available_windows: usize,
    window_aec_available: bool,
    mic_dbfs_sum: f32,
    mic_dbfs_count: usize,
    speaker_dbfs_sum: f32,
    speaker_dbfs_count: usize,
    max_mic_dbfs: Option<f32>,
    max_speaker_dbfs: Option<f32>,
}

struct PlaybackGuard(Option<std::sync::mpsc::Sender<()>>);

impl Drop for PlaybackGuard {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.0.take() {
            let _ = stop_tx.send(());
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    validate_args(&args)?;

    let started_unix_secs = unix_secs();
    let root_out_dir = args.out_dir.clone().unwrap_or_else(|| {
        PathBuf::from("target/provider-meeting-probe").join(started_unix_secs.to_string())
    });
    std::fs::create_dir_all(&root_out_dir)
        .with_context(|| format!("create output directory {}", root_out_dir.display()))?;

    let playback_file = args
        .playback_file
        .clone()
        .unwrap_or_else(|| PathBuf::from(hypr_data::english_1::AUDIO_PATH));
    let expected_phrases = expected_phrases(&args);

    let mut results = Vec::new();
    for volume in &args.playback_volumes {
        for provider in &args.providers {
            let run_dir = root_out_dir.join(run_dir_name(*provider, *volume));
            std::fs::create_dir_all(&run_dir)
                .with_context(|| format!("create run directory {}", run_dir.display()))?;

            let summary = match run_provider_session(
                &args,
                *provider,
                *volume,
                &playback_file,
                &expected_phrases,
                &run_dir,
            )
            .await
            {
                Ok(summary) => summary,
                Err(error) => failed_summary(&args, *provider, *volume, &run_dir, error),
            };
            results.push(summary);
        }
    }

    let matrix = MatrixSummary {
        started_unix_secs,
        duration_secs_requested: args.duration_secs,
        sample_rate: args.sample_rate,
        playback_file: path_string(&playback_file),
        expected_phrases,
        results,
    };

    let summary_path = root_out_dir.join("summary.json");
    std::fs::write(&summary_path, serde_json::to_string_pretty(&matrix)?)?;
    println!("{}", summary_path.display());

    if !args.allow_failures
        && matrix
            .results
            .iter()
            .any(|result| matches!(result.status, RunStatus::Failed))
    {
        anyhow::bail!(
            "provider_meeting_probe_failed: see {}",
            summary_path.display()
        );
    }

    Ok(())
}

async fn run_provider_session(
    args: &Args,
    provider: ProbeProvider,
    playback_volume: f32,
    playback_file: &Path,
    expected_phrases: &[String],
    run_dir: &Path,
) -> anyhow::Result<ProviderRunSummary> {
    let model = model_for(args, provider)?;

    if let Some(reason) = skip_reason(args, provider, &model) {
        return Ok(ProviderRunSummary {
            provider,
            model,
            playback_volume,
            status: RunStatus::Skipped,
            skip_reason: Some(reason),
            error: None,
            out_dir: path_string(run_dir),
            transcript_jsonl: None,
            metrics_jsonl: None,
            duration_secs_captured: 0.0,
            audio: None,
            transcript: None,
            verdict: None,
        });
    }

    let transcript_path = run_dir.join("transcript.jsonl");
    let metrics_path = run_dir.join("metrics.jsonl");
    let mut metrics_writer = BufWriter::new(File::create(&metrics_path)?);
    let mut runner =
        start_provider_runner(args, provider, &model, &transcript_path, expected_phrases).await?;
    let chunk_size = hypr_audio_utils::chunk_size_for_stt(args.sample_rate);
    let mut stream = AudioInput::from_mic_and_speaker(CaptureConfig {
        sample_rate: args.sample_rate,
        chunk_size,
        mic_device: args.mic_device.clone(),
        enable_aec: !args.no_aec,
    })?;

    tokio::time::sleep(Duration::from_millis(500)).await;
    let _playback = PlaybackGuard(Some(start_playback(
        playback_file.to_path_buf(),
        playback_volume,
    )?));

    let mut audio = AudioAccumulator::default();
    let target_samples = args.duration_secs as usize * args.sample_rate as usize;
    let window_samples = args.window_secs.max(1) as usize * args.sample_rate as usize;

    while audio.captured_samples < target_samples {
        let Some(frame) = stream.next().await else {
            break;
        };
        let frame = frame?;
        let mic_samples = frame.preferred_mic();
        let speaker_samples = frame.raw_speaker.clone();
        if frame.aec_mic.is_some() {
            audio.window_aec_available = true;
        }

        runner
            .tx
            .send(MixedMessage::Audio((
                f32_to_i16_le_bytes(&mic_samples),
                f32_to_i16_le_bytes(&speaker_samples),
            )))
            .await
            .context("provider stream ended while sending audio")?;

        audio.push(&mic_samples, &speaker_samples);

        while audio.window_mic.len() >= window_samples {
            let metrics =
                audio.drain_window(window_samples, args.sample_rate, args.speaker_active_dbfs);
            serde_json::to_writer(&mut metrics_writer, &metrics)?;
            metrics_writer.write_all(b"\n")?;

            let preflight_windows =
                (args.speaker_preflight_secs / args.window_secs.max(1)).max(1) as usize;
            if !args.allow_silent_speaker
                && audio.window_count >= preflight_windows
                && audio.speaker_active_windows == 0
            {
                anyhow::bail!(
                    "speaker_capture_silent: no speaker activity after {}s at playback volume {}; check system-audio permission/output routing, or pass --allow-silent-speaker",
                    args.speaker_preflight_secs,
                    playback_volume,
                );
            }
        }
    }

    metrics_writer.flush()?;
    runner.finalize.finalize().await;
    drop(runner.tx);

    let transcript = tokio::time::timeout(
        Duration::from_secs(DEFAULT_COLLECTOR_TIMEOUT_SECS),
        &mut runner.collector,
    )
    .await
    .context("timed out waiting for provider transcript collector")?
    .context("provider transcript collector panicked")??;

    let audio_summary = audio.summary();
    let verdict = evaluate_verdict(args, expected_phrases, &audio_summary, &transcript);
    let status = if verdict.passed {
        RunStatus::Passed
    } else {
        RunStatus::Failed
    };

    Ok(ProviderRunSummary {
        provider,
        model,
        playback_volume,
        status,
        skip_reason: None,
        error: None,
        out_dir: path_string(run_dir),
        transcript_jsonl: Some(path_string(&transcript_path)),
        metrics_jsonl: Some(path_string(&metrics_path)),
        duration_secs_captured: audio.captured_samples as f64 / args.sample_rate as f64,
        audio: Some(audio_summary),
        transcript: Some(transcript),
        verdict: Some(verdict),
    })
}

async fn start_provider_runner(
    args: &Args,
    provider: ProbeProvider,
    model: &str,
    transcript_path: &Path,
    expected_phrases: &[String],
) -> anyhow::Result<ProviderRunner> {
    let (tx, rx) = tokio::sync::mpsc::channel(DEFAULT_PROVIDER_QUEUE);
    let outbound = ReceiverStream::new(rx);

    match provider {
        ProbeProvider::Parakeet => {
            let soniqo_model = model.parse::<hypr_transcribe_soniqo::SoniqoModel>()?;
            let client = owhisper_client::LocalSoniqoLiveClient::new(soniqo_model);
            let (stream, handle) = client.from_realtime_audio_dual(outbound).await?;
            let collector = spawn_transcript_collector(
                provider,
                stream,
                transcript_path.to_path_buf(),
                expected_phrases.to_vec(),
            );
            Ok(ProviderRunner {
                tx,
                finalize: ProbeFinalizeHandle::Parakeet(handle),
                collector,
            })
        }
        ProbeProvider::Deepgram => {
            let api_key = std::env::var(Provider::Deepgram.env_key_name())?;
            let client = ListenClient::builder()
                .adapter::<DeepgramAdapter>()
                .api_base(
                    args.deepgram_api_base
                        .as_deref()
                        .unwrap_or_else(|| Provider::Deepgram.default_api_base()),
                )
                .api_key(api_key)
                .params(listen_params(model, args.sample_rate))
                .build_dual()
                .await;
            let (stream, handle) = client.from_realtime_audio(outbound).await?;
            let collector = spawn_transcript_collector(
                provider,
                stream,
                transcript_path.to_path_buf(),
                expected_phrases.to_vec(),
            );
            Ok(ProviderRunner {
                tx,
                finalize: ProbeFinalizeHandle::Cloud(handle),
                collector,
            })
        }
        ProbeProvider::Soniox => {
            let api_key = std::env::var(Provider::Soniox.env_key_name())?;
            let client = ListenClient::builder()
                .adapter::<SonioxAdapter>()
                .api_base(
                    args.soniox_api_base
                        .as_deref()
                        .unwrap_or_else(|| Provider::Soniox.default_api_base()),
                )
                .api_key(api_key)
                .params(listen_params(model, args.sample_rate))
                .build_dual()
                .await;
            let (stream, handle) = client.from_realtime_audio(outbound).await?;
            let collector = spawn_transcript_collector(
                provider,
                stream,
                transcript_path.to_path_buf(),
                expected_phrases.to_vec(),
            );
            Ok(ProviderRunner {
                tx,
                finalize: ProbeFinalizeHandle::Cloud(handle),
                collector,
            })
        }
    }
}

fn spawn_transcript_collector<E>(
    provider: ProbeProvider,
    mut stream: impl Stream<Item = Result<StreamResponse, E>> + Send + Unpin + 'static,
    transcript_path: PathBuf,
    expected_phrases: Vec<String>,
) -> tokio::task::JoinHandle<anyhow::Result<TranscriptSummary>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let mut writer = BufWriter::new(File::create(&transcript_path)?);
        let mut events = Vec::new();
        let mut errors = Vec::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    serde_json::to_writer(
                        &mut writer,
                        &TranscriptRecord {
                            provider,
                            received_unix_millis: unix_millis(),
                            response: &response,
                        },
                    )?;
                    writer.write_all(b"\n")?;

                    match response {
                        StreamResponse::TranscriptResponse {
                            start,
                            is_final,
                            channel,
                            channel_index,
                            ..
                        } => {
                            let text = channel
                                .alternatives
                                .first()
                                .map(|alt| alt.transcript.trim().to_string())
                                .unwrap_or_default();
                            if !text.is_empty() {
                                events.push(TranscriptEvent {
                                    start,
                                    channel: channel_index.first().copied(),
                                    is_final,
                                    text,
                                });
                            }
                        }
                        StreamResponse::ErrorResponse { error_message, .. } => {
                            errors.push(error_message);
                        }
                        _ => {}
                    }
                }
                Err(error) => {
                    errors.push(error.to_string());
                }
            }
        }

        writer.flush()?;
        Ok(summarize_transcript(&events, errors, &expected_phrases))
    })
}

fn summarize_transcript(
    events: &[TranscriptEvent],
    errors: Vec<String>,
    expected_phrases: &[String],
) -> TranscriptSummary {
    let mut summary = TranscriptSummary {
        total_results: events.len(),
        final_results: events.iter().filter(|event| event.is_final).count(),
        mic_results: events
            .iter()
            .filter(|event| event.channel == Some(0))
            .count(),
        speaker_results: events
            .iter()
            .filter(|event| event.channel == Some(1))
            .count(),
        mic_final_results: events
            .iter()
            .filter(|event| event.channel == Some(0) && event.is_final)
            .count(),
        speaker_final_results: events
            .iter()
            .filter(|event| event.channel == Some(1) && event.is_final)
            .count(),
        errors,
        ..Default::default()
    };

    let mic_text = joined_text(events, Some(0));
    let speaker_text = joined_text(events, Some(1));
    summary.mic_word_count = normalized_tokens(&mic_text).len();
    summary.speaker_word_count = normalized_tokens(&speaker_text).len();
    summary.mic_expected_phrase_hits = phrase_hits(&mic_text, expected_phrases);
    summary.speaker_expected_phrase_hits = phrase_hits(&speaker_text, expected_phrases);
    summary.mic_text_preview = preview(&mic_text);
    summary.speaker_text_preview = preview(&speaker_text);
    summary.near_duplicate_cross_channel_results = count_cross_channel_duplicates(events);

    summary
}

fn evaluate_verdict(
    args: &Args,
    expected_phrases: &[String],
    audio: &AudioSummary,
    transcript: &TranscriptSummary,
) -> Verdict {
    let mut reasons = Vec::new();

    if audio.speaker_active_windows == 0 {
        reasons.push("speaker capture had no active windows".to_string());
    }
    if transcript.speaker_results < args.min_speaker_results {
        reasons.push(format!(
            "speaker/system transcript results {} below minimum {}",
            transcript.speaker_results, args.min_speaker_results
        ));
    }
    if transcript.mic_results > args.max_mic_results {
        reasons.push(format!(
            "mic/user transcript results {} above maximum {}",
            transcript.mic_results, args.max_mic_results
        ));
    }
    if transcript.near_duplicate_cross_channel_results > args.max_cross_channel_duplicates {
        reasons.push(format!(
            "cross-channel duplicate results {} above maximum {}",
            transcript.near_duplicate_cross_channel_results, args.max_cross_channel_duplicates
        ));
    }
    if !transcript.errors.is_empty() {
        reasons.push(format!(
            "provider emitted {} errors",
            transcript.errors.len()
        ));
    }

    if !expected_phrases.is_empty() {
        if transcript.speaker_expected_phrase_hits < args.min_speaker_phrase_hits {
            reasons.push(format!(
                "speaker/system expected phrase hits {} below minimum {}",
                transcript.speaker_expected_phrase_hits, args.min_speaker_phrase_hits
            ));
        }
        if transcript.mic_expected_phrase_hits > args.max_mic_phrase_hits {
            reasons.push(format!(
                "mic/user expected phrase hits {} above maximum {}",
                transcript.mic_expected_phrase_hits, args.max_mic_phrase_hits
            ));
        }
    }

    Verdict {
        passed: reasons.is_empty(),
        reasons,
    }
}

impl AudioAccumulator {
    fn push(&mut self, mic: &[f32], speaker: &[f32]) {
        self.window_mic.extend_from_slice(mic);
        self.window_speaker.extend_from_slice(speaker);
        self.captured_samples += mic.len().min(speaker.len());
    }

    fn drain_window(
        &mut self,
        window_samples: usize,
        sample_rate: u32,
        speaker_active_dbfs: f32,
    ) -> WindowMetrics {
        let mic = drain_exact(&mut self.window_mic, window_samples);
        let speaker = drain_exact(&mut self.window_speaker, window_samples);
        let mic_level = level_metrics(&mic);
        let speaker_level = level_metrics(&speaker);
        let aec_available = self.window_aec_available;
        self.window_aec_available = false;

        if aec_available {
            self.aec_available_windows += 1;
        }
        if speaker_level.dbfs.unwrap_or(-120.0) >= speaker_active_dbfs {
            self.speaker_active_windows += 1;
        }
        if let Some(dbfs) = mic_level.dbfs {
            self.mic_dbfs_sum += dbfs;
            self.mic_dbfs_count += 1;
            self.max_mic_dbfs = Some(self.max_mic_dbfs.map(|v| v.max(dbfs)).unwrap_or(dbfs));
        }
        if let Some(dbfs) = speaker_level.dbfs {
            self.speaker_dbfs_sum += dbfs;
            self.speaker_dbfs_count += 1;
            self.max_speaker_dbfs =
                Some(self.max_speaker_dbfs.map(|v| v.max(dbfs)).unwrap_or(dbfs));
        }

        let window_index = self.window_count;
        self.window_count += 1;

        WindowMetrics {
            window_index,
            start_secs: window_index as f64 * window_samples as f64 / sample_rate as f64,
            end_secs: (window_index + 1) as f64 * window_samples as f64 / sample_rate as f64,
            aec_available,
            mic: mic_level,
            speaker: speaker_level,
        }
    }

    fn summary(&self) -> AudioSummary {
        AudioSummary {
            window_count: self.window_count,
            speaker_active_windows: self.speaker_active_windows,
            aec_available_windows: self.aec_available_windows,
            avg_mic_dbfs: average(self.mic_dbfs_sum, self.mic_dbfs_count),
            avg_speaker_dbfs: average(self.speaker_dbfs_sum, self.speaker_dbfs_count),
            max_mic_dbfs: self.max_mic_dbfs,
            max_speaker_dbfs: self.max_speaker_dbfs,
        }
    }
}

fn validate_args(args: &Args) -> anyhow::Result<()> {
    if args.providers.is_empty() {
        anyhow::bail!("at least one provider is required");
    }
    if args.playback_volumes.is_empty() {
        anyhow::bail!("at least one playback volume is required");
    }
    for volume in &args.playback_volumes {
        if !volume.is_finite() || *volume < 0.0 {
            anyhow::bail!("playback volumes must be finite non-negative values");
        }
    }
    if args.sample_rate == 0 {
        anyhow::bail!("sample_rate must be positive");
    }

    Ok(())
}

fn listen_params(model: &str, sample_rate: u32) -> owhisper_interface::ListenParams {
    owhisper_interface::ListenParams {
        model: Some(model.to_string()),
        languages: vec![hypr_language::ISO639::En.into()],
        sample_rate,
        num_speakers: Some(2),
        max_speakers: Some(2),
        ..Default::default()
    }
}

fn model_for(args: &Args, provider: ProbeProvider) -> anyhow::Result<String> {
    let model = match provider {
        ProbeProvider::Parakeet => args
            .parakeet_model
            .as_deref()
            .unwrap_or(hypr_transcribe_soniqo::SoniqoModel::ParakeetStreaming.as_str()),
        ProbeProvider::Deepgram => args
            .deepgram_model
            .as_deref()
            .unwrap_or_else(|| Provider::Deepgram.default_live_model()),
        ProbeProvider::Soniox => args
            .soniox_model
            .as_deref()
            .unwrap_or_else(|| Provider::Soniox.default_live_model()),
    };

    if provider == ProbeProvider::Parakeet {
        model.parse::<hypr_transcribe_soniqo::SoniqoModel>()?;
    }

    Ok(model.to_string())
}

fn skip_reason(_args: &Args, provider: ProbeProvider, model: &str) -> Option<String> {
    match provider {
        ProbeProvider::Parakeet => {
            let model = model.parse::<hypr_transcribe_soniqo::SoniqoModel>().ok()?;
            if !model.supports_live_on_current_platform() {
                return Some(format!(
                    "{} is not available for live transcription on this platform",
                    model
                ));
            }

            match hypr_transcribe_soniqo::is_model_downloaded(model) {
                Ok(true) => None,
                Ok(false) => Some(format!("{} is not downloaded", model)),
                Err(error) => Some(format!("{} availability check failed: {}", model, error)),
            }
        }
        ProbeProvider::Deepgram => missing_env_reason(Provider::Deepgram.env_key_name()),
        ProbeProvider::Soniox => missing_env_reason(Provider::Soniox.env_key_name()),
    }
}

fn missing_env_reason(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|_| ())
        .is_none()
        .then(|| format!("{} is not set", name))
}

fn expected_phrases(args: &Args) -> Vec<String> {
    if !args.expected_phrases.is_empty() {
        return args.expected_phrases.clone();
    }
    if args.playback_file.is_some() {
        return Vec::new();
    }

    vec![
        "Maybe this is me talking to the audience".to_string(),
        "so many messages".to_string(),
        "complementary skills".to_string(),
    ]
}

fn failed_summary(
    args: &Args,
    provider: ProbeProvider,
    playback_volume: f32,
    run_dir: &Path,
    error: anyhow::Error,
) -> ProviderRunSummary {
    ProviderRunSummary {
        provider,
        model: model_for(args, provider).unwrap_or_else(|_| "unknown".to_string()),
        playback_volume,
        status: RunStatus::Failed,
        skip_reason: None,
        error: Some(error.to_string()),
        out_dir: path_string(run_dir),
        transcript_jsonl: None,
        metrics_jsonl: None,
        duration_secs_captured: 0.0,
        audio: None,
        transcript: None,
        verdict: None,
    }
}

fn joined_text(events: &[TranscriptEvent], channel: Option<i32>) -> String {
    events
        .iter()
        .filter(|event| event.channel == channel)
        .map(|event| event.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn count_cross_channel_duplicates(events: &[TranscriptEvent]) -> usize {
    let mut near_duplicates = 0usize;
    for (idx, event) in events.iter().enumerate() {
        if event.channel.is_none() {
            continue;
        }

        let duplicate = events[..idx].iter().rev().take(12).any(|previous| {
            previous.channel != event.channel
                && previous.channel.is_some()
                && (event.start - previous.start).abs() <= 3.0
                && shares_phrase(&event.text, &previous.text)
        });

        if duplicate {
            near_duplicates += 1;
        }
    }
    near_duplicates
}

fn phrase_hits(text: &str, phrases: &[String]) -> usize {
    let text_tokens = normalized_tokens(text);
    phrases
        .iter()
        .filter(|phrase| contains_phrase_tokens(&text_tokens, &normalized_tokens(phrase)))
        .count()
}

fn contains_phrase_tokens(text_tokens: &[String], phrase_tokens: &[String]) -> bool {
    if phrase_tokens.is_empty() || text_tokens.len() < phrase_tokens.len() {
        return false;
    }

    text_tokens
        .windows(phrase_tokens.len())
        .any(|window| window == phrase_tokens)
}

fn shares_phrase(left: &str, right: &str) -> bool {
    let left_tokens = normalized_tokens(left);
    let right_tokens = normalized_tokens(right);
    if left_tokens.len() < 3 || right_tokens.len() < 3 {
        return left_tokens == right_tokens && !left_tokens.is_empty();
    }

    let left_phrases = token_phrases(&left_tokens);
    token_phrases(&right_tokens)
        .into_iter()
        .any(|phrase| left_phrases.contains(&phrase))
}

fn normalized_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|token| {
            token
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

fn token_phrases(tokens: &[String]) -> HashSet<String> {
    tokens
        .windows(3)
        .map(|window| window.join(" "))
        .collect::<HashSet<_>>()
}

fn preview(text: &str) -> String {
    const MAX_CHARS: usize = 800;
    if text.chars().count() <= MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(MAX_CHARS).collect::<String>()
}

fn f32_to_i16_le_bytes(samples: &[f32]) -> Bytes {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let sample = if sample.is_finite() { *sample } else { 0.0 };
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    Bytes::from(bytes)
}

fn drain_exact(samples: &mut Vec<f32>, len: usize) -> Vec<f32> {
    samples.drain(..len.min(samples.len())).collect()
}

fn level_metrics(samples: &[f32]) -> LevelMetrics {
    let rms = if samples.is_empty() {
        0.0
    } else {
        let energy = samples
            .iter()
            .filter(|sample| sample.is_finite())
            .map(|sample| sample * sample)
            .sum::<f32>();
        (energy / samples.len() as f32).sqrt()
    };
    let peak = samples
        .iter()
        .filter(|sample| sample.is_finite())
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let dbfs = if rms > 0.0 {
        Some(20.0 * rms.log10())
    } else {
        None
    };

    LevelMetrics { rms, dbfs, peak }
}

fn start_playback(path: PathBuf, volume: f32) -> anyhow::Result<std::sync::mpsc::Sender<()>> {
    start_playback_impl(path, volume)
}

#[cfg(target_os = "macos")]
fn start_playback_impl(path: PathBuf, volume: f32) -> anyhow::Result<std::sync::mpsc::Sender<()>> {
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            let mut child = spawn_afplay(&path, volume)?;
            let _ = ready_tx.send(Ok(()));

            loop {
                match stop_rx.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        stop_playback_child(&mut child);
                        return Ok(());
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }

                if child.try_wait()?.is_some() {
                    child = spawn_afplay(&path, volume)?;
                }

                std::thread::sleep(Duration::from_millis(100));
            }
        })();

        if let Err(error) = result {
            let _ = ready_tx.send(Err(error.to_string()));
        }
    });

    match ready_rx.recv().context("wait for playback startup")? {
        Ok(()) => Ok(stop_tx),
        Err(error) => Err(anyhow::anyhow!(error)),
    }
}

#[cfg(target_os = "macos")]
fn spawn_afplay(path: &Path, volume: f32) -> anyhow::Result<Child> {
    Command::new("afplay")
        .arg("--volume")
        .arg(volume.to_string())
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("start afplay for {}", path.display()))
}

#[cfg(target_os = "macos")]
fn stop_playback_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[cfg(not(target_os = "macos"))]
fn start_playback_impl(path: PathBuf, volume: f32) -> anyhow::Result<std::sync::mpsc::Sender<()>> {
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            use rodio::Source;

            let stream = rodio::stream::DeviceSinkBuilder::open_default_sink()?;
            let file = File::open(&path)?;
            let source = rodio::Decoder::try_from(file)?;
            let player = rodio::Player::connect_new(stream.mixer());
            player.set_volume(volume);
            player.append(source.repeat_infinite());
            let _ = ready_tx.send(Ok(()));
            let _ = stop_rx.recv();
            player.stop();
            Ok(())
        })();

        if let Err(error) = result {
            let _ = ready_tx.send(Err(error.to_string()));
        }
    });

    match ready_rx.recv().context("wait for playback startup")? {
        Ok(()) => Ok(stop_tx),
        Err(error) => Err(anyhow::anyhow!(error)),
    }
}

fn run_dir_name(provider: ProbeProvider, volume: f32) -> String {
    format!("{}-volume-{:.2}", provider, volume).replace('.', "p")
}

fn average(sum: f32, count: usize) -> Option<f32> {
    (count > 0).then(|| sum / count as f32)
}

fn path_string(path: &Path) -> String {
    path.display().to_string()
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis()
        .min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_phrase_hits_after_normalization() {
        let text = "Maybe this is me, talking to the audience a little bit.";
        let phrases = vec!["maybe this is me talking to the audience".to_string()];

        assert_eq!(phrase_hits(text, &phrases), 1);
    }

    #[test]
    fn detects_cross_channel_duplicate_phrase() {
        let events = vec![
            TranscriptEvent {
                start: 1.0,
                channel: Some(1),
                is_final: true,
                text: "queries yielding a statistic portrait".to_string(),
            },
            TranscriptEvent {
                start: 2.0,
                channel: Some(0),
                is_final: false,
                text: "of queries yielding a statistic".to_string(),
            },
        ];

        assert_eq!(count_cross_channel_duplicates(&events), 1);
    }

    #[test]
    fn converts_f32_samples_to_pcm16() {
        let bytes = f32_to_i16_le_bytes(&[-1.0, 0.0, 1.0]);
        let samples = bytes
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();

        assert_eq!(samples, vec![-32767, 0, 32767]);
    }
}
