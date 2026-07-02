use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use audio_actual::AudioInput;
use clap::Parser;
use futures_util::StreamExt;
use hypr_audio::{CaptureConfig, CaptureFrame};
use serde::Serialize;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_t = 900)]
    duration_secs: u64,
    #[arg(long, default_value_t = 16_000)]
    sample_rate: u32,
    #[arg(long)]
    mic_device: Option<String>,
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long)]
    playback_file: Option<PathBuf>,
    #[arg(long, default_value_t = 1.0)]
    playback_volume: f32,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    transcript_jsonl: Option<PathBuf>,
    #[arg(long, default_value_t = 1)]
    window_secs: u64,
    #[arg(long, default_value_t = 250)]
    max_lag_ms: u32,
    #[arg(long)]
    no_aec: bool,
    #[arg(long)]
    allow_silent_speaker: bool,
    #[arg(long, default_value_t = 5)]
    speaker_preflight_secs: u64,
}

#[derive(Default)]
struct RunBuffers {
    raw_mic: Vec<f32>,
    processed_mic: Vec<f32>,
    speaker: Vec<f32>,
    window_raw_mic: Vec<f32>,
    window_processed_mic: Vec<f32>,
    window_speaker: Vec<f32>,
    window_aec_available: bool,
    aec_available_windows: usize,
}

#[derive(Serialize)]
struct RunSummary {
    started_unix_secs: u64,
    duration_secs_requested: u64,
    duration_secs_captured: f64,
    sample_rate: u32,
    chunk_size: usize,
    aec_enabled: bool,
    provider: Option<String>,
    model: Option<String>,
    playback_file: Option<String>,
    playback_engine: Option<String>,
    playback_volume: Option<f32>,
    transcript_jsonl: Option<String>,
    raw_mic_wav: String,
    processed_mic_wav: String,
    speaker_wav: String,
    metrics_jsonl: String,
    window_count: usize,
    aec_available_windows: usize,
    echo_likely_windows: usize,
    speaker_active_windows: usize,
    avg_processed_speaker_correlation: Option<f32>,
    avg_aec_attenuation_db: Option<f32>,
    transcript: Option<TranscriptSummary>,
}

#[derive(Serialize)]
struct WindowMetrics {
    window_index: usize,
    start_secs: f64,
    end_secs: f64,
    aec_available: bool,
    raw_mic: LevelMetrics,
    processed_mic: LevelMetrics,
    speaker: LevelMetrics,
    raw_mic_speaker_correlation: Option<f32>,
    raw_mic_speaker_lag_ms: Option<f32>,
    processed_mic_speaker_correlation: Option<f32>,
    processed_mic_speaker_lag_ms: Option<f32>,
    aec_attenuation_db: Option<f32>,
    echo_likely: bool,
}

#[derive(Serialize)]
struct LevelMetrics {
    rms: f32,
    dbfs: Option<f32>,
    peak: f32,
}

#[derive(Serialize)]
struct TranscriptSummary {
    path: String,
    total_results: usize,
    final_results: usize,
    channel_0_results: usize,
    channel_1_results: usize,
    near_duplicate_cross_channel_results: usize,
}

#[derive(Debug)]
struct TranscriptEvent {
    start: f64,
    channel: Option<i64>,
    is_final: bool,
    text: String,
}

#[derive(Clone, Copy)]
struct Correlation {
    value: f32,
    lag_samples: isize,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if !args.playback_volume.is_finite() || args.playback_volume < 0.0 {
        anyhow::bail!("playback_volume must be a finite non-negative value");
    }

    let started_unix_secs = unix_secs();
    let out_dir = args
        .out_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("target/aec-probe").join(started_unix_secs.to_string()));
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("create output directory {}", out_dir.display()))?;

    let chunk_size = hypr_audio_utils::chunk_size_for_stt(args.sample_rate);
    let mut stream = AudioInput::from_mic_and_speaker(CaptureConfig {
        sample_rate: args.sample_rate,
        chunk_size,
        mic_device: args.mic_device.clone(),
        enable_aec: !args.no_aec,
    })?;

    tokio::time::sleep(Duration::from_millis(500)).await;
    let _playback = match args.playback_file.clone() {
        Some(path) => Some(start_playback(path, args.playback_volume)?),
        None => None,
    };

    let metrics_path = out_dir.join("metrics.jsonl");
    let mut metrics_writer = BufWriter::new(File::create(&metrics_path)?);
    let mut buffers = RunBuffers::default();
    let mut window_count = 0usize;
    let mut echo_likely_windows = 0usize;
    let mut speaker_active_windows = 0usize;
    let mut processed_corr_sum = 0.0f32;
    let mut processed_corr_count = 0usize;
    let mut attenuation_sum = 0.0f32;
    let mut attenuation_count = 0usize;

    let target_samples = args.duration_secs as usize * args.sample_rate as usize;
    let window_samples = args.window_secs.max(1) as usize * args.sample_rate as usize;
    let max_lag_samples = ((args.sample_rate as u64 * args.max_lag_ms as u64) / 1000) as isize;

    while buffers.raw_mic.len() < target_samples {
        let Some(frame) = stream.next().await else {
            break;
        };
        let frame = frame?;
        buffers.push_frame(frame);

        while buffers.window_raw_mic.len() >= window_samples {
            let metrics = drain_window_metrics(
                &mut buffers,
                window_samples,
                window_count,
                args.sample_rate,
                max_lag_samples,
            );

            if metrics.echo_likely {
                echo_likely_windows += 1;
            }
            if metrics.speaker.peak > 1e-6 {
                speaker_active_windows += 1;
            }
            if let Some(value) = metrics.processed_mic_speaker_correlation {
                processed_corr_sum += value;
                processed_corr_count += 1;
            }
            if let Some(value) = metrics.aec_attenuation_db {
                attenuation_sum += value;
                attenuation_count += 1;
            }

            serde_json::to_writer(&mut metrics_writer, &metrics)?;
            metrics_writer.write_all(b"\n")?;
            window_count += 1;

            let preflight_windows =
                (args.speaker_preflight_secs / args.window_secs.max(1)).max(1) as usize;
            if args.playback_file.is_some()
                && !args.allow_silent_speaker
                && window_count >= preflight_windows
                && speaker_active_windows == 0
            {
                anyhow::bail!(
                    "speaker_capture_silent: no nonzero speaker samples after {}s with playback enabled; check system-audio permission or output routing, or pass --allow-silent-speaker to keep recording",
                    args.speaker_preflight_secs
                );
            }
        }
    }
    metrics_writer.flush()?;

    let raw_mic_path = out_dir.join("raw_mic.wav");
    let processed_mic_path = out_dir.join("processed_mic.wav");
    let speaker_path = out_dir.join("speaker.wav");
    write_wav(&raw_mic_path, &buffers.raw_mic, args.sample_rate)?;
    write_wav(
        &processed_mic_path,
        &buffers.processed_mic,
        args.sample_rate,
    )?;
    write_wav(&speaker_path, &buffers.speaker, args.sample_rate)?;

    let transcript = args
        .transcript_jsonl
        .as_ref()
        .map(|path| analyze_transcript_jsonl(path))
        .transpose()?;

    let summary = RunSummary {
        started_unix_secs,
        duration_secs_requested: args.duration_secs,
        duration_secs_captured: buffers.raw_mic.len() as f64 / args.sample_rate as f64,
        sample_rate: args.sample_rate,
        chunk_size,
        aec_enabled: !args.no_aec,
        provider: args.provider,
        model: args.model,
        playback_file: args.playback_file.as_ref().map(|path| path_string(path)),
        playback_engine: args
            .playback_file
            .as_ref()
            .map(|_| playback_engine().to_string()),
        playback_volume: args.playback_file.as_ref().map(|_| args.playback_volume),
        transcript_jsonl: args.transcript_jsonl.as_ref().map(|path| path_string(path)),
        raw_mic_wav: path_string(&raw_mic_path),
        processed_mic_wav: path_string(&processed_mic_path),
        speaker_wav: path_string(&speaker_path),
        metrics_jsonl: path_string(&metrics_path),
        window_count,
        aec_available_windows: buffers.aec_available_windows,
        echo_likely_windows,
        speaker_active_windows,
        avg_processed_speaker_correlation: average(processed_corr_sum, processed_corr_count),
        avg_aec_attenuation_db: average(attenuation_sum, attenuation_count),
        transcript,
    };

    let summary_path = out_dir.join("summary.json");
    std::fs::write(&summary_path, serde_json::to_string_pretty(&summary)?)?;
    println!("{}", summary_path.display());

    Ok(())
}

impl RunBuffers {
    fn push_frame(&mut self, frame: CaptureFrame) {
        let processed_mic = frame
            .aec_mic
            .as_ref()
            .map(|samples| samples.as_ref())
            .unwrap_or_else(|| frame.raw_mic.as_ref());

        if frame.aec_mic.is_some() {
            self.window_aec_available = true;
        }

        self.window_raw_mic.extend_from_slice(&frame.raw_mic);
        self.window_processed_mic.extend_from_slice(processed_mic);
        self.window_speaker.extend_from_slice(&frame.raw_speaker);
        self.raw_mic.extend_from_slice(&frame.raw_mic);
        self.processed_mic.extend_from_slice(processed_mic);
        self.speaker.extend_from_slice(&frame.raw_speaker);
    }
}

fn drain_window_metrics(
    buffers: &mut RunBuffers,
    window_samples: usize,
    window_index: usize,
    sample_rate: u32,
    max_lag_samples: isize,
) -> WindowMetrics {
    let aec_available = buffers.window_aec_available;
    buffers.window_aec_available = false;
    if aec_available {
        buffers.aec_available_windows += 1;
    }

    let raw_mic = drain_exact(&mut buffers.window_raw_mic, window_samples);
    let processed_mic = drain_exact(&mut buffers.window_processed_mic, window_samples);
    let speaker = drain_exact(&mut buffers.window_speaker, window_samples);
    let raw_level = level_metrics(&raw_mic);
    let processed_level = level_metrics(&processed_mic);
    let speaker_level = level_metrics(&speaker);
    let raw_corr = best_abs_correlation(&raw_mic, &speaker, max_lag_samples);
    let processed_corr = best_abs_correlation(&processed_mic, &speaker, max_lag_samples);
    let attenuation = match (raw_level.dbfs, processed_level.dbfs) {
        (Some(raw), Some(processed)) => Some(raw - processed),
        _ => None,
    };

    let echo_likely = speaker_level.dbfs.unwrap_or(-120.0) > -45.0
        && processed_level.dbfs.unwrap_or(-120.0) > -55.0
        && processed_corr.map(|c| c.value >= 0.55).unwrap_or(false);

    WindowMetrics {
        window_index,
        start_secs: window_index as f64 * window_samples as f64 / sample_rate as f64,
        end_secs: (window_index + 1) as f64 * window_samples as f64 / sample_rate as f64,
        aec_available,
        raw_mic: raw_level,
        processed_mic: processed_level,
        speaker: speaker_level,
        raw_mic_speaker_correlation: raw_corr.map(|c| c.value),
        raw_mic_speaker_lag_ms: raw_corr.map(|c| lag_ms(c.lag_samples, sample_rate)),
        processed_mic_speaker_correlation: processed_corr.map(|c| c.value),
        processed_mic_speaker_lag_ms: processed_corr.map(|c| lag_ms(c.lag_samples, sample_rate)),
        aec_attenuation_db: attenuation,
        echo_likely,
    }
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

fn best_abs_correlation(mic: &[f32], speaker: &[f32], max_lag: isize) -> Option<Correlation> {
    let len = mic.len().min(speaker.len());
    if len < 512 {
        return None;
    }

    let max_lag = max_lag.min((len - 512) as isize).max(0);
    let mut best = correlation_at_lag(&mic[..len], &speaker[..len], 0);
    let mut lag = 80;

    while lag <= max_lag {
        for lag in [-lag, lag] {
            if let Some(candidate) = correlation_at_lag(&mic[..len], &speaker[..len], lag) {
                if best
                    .map(|current| candidate.value > current.value)
                    .unwrap_or(true)
                {
                    best = Some(candidate);
                }
            }
        }
        lag += 80;
    }

    best
}

fn correlation_at_lag(mic: &[f32], speaker: &[f32], lag: isize) -> Option<Correlation> {
    let len = mic.len().min(speaker.len());
    let (mic_start, speaker_start) = if lag >= 0 {
        (lag as usize, 0)
    } else {
        (0, lag.unsigned_abs())
    };
    let overlap = len.saturating_sub(mic_start.max(speaker_start));
    if overlap < 512 {
        return None;
    }

    let mut mic_energy = 0.0;
    let mut speaker_energy = 0.0;
    let mut cross_energy = 0.0;
    for idx in 0..overlap {
        let mic_sample = mic[mic_start + idx];
        let speaker_sample = speaker[speaker_start + idx];
        mic_energy += mic_sample * mic_sample;
        speaker_energy += speaker_sample * speaker_sample;
        cross_energy += mic_sample * speaker_sample;
    }

    if mic_energy <= f32::EPSILON || speaker_energy <= f32::EPSILON {
        return None;
    }

    Some(Correlation {
        value: cross_energy.abs() / (mic_energy * speaker_energy).sqrt().max(1e-6),
        lag_samples: lag,
    })
}

fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(())
}

fn start_playback(path: PathBuf, volume: f32) -> anyhow::Result<std::sync::mpsc::Sender<()>> {
    start_playback_impl(path, volume)
}

fn playback_engine() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "afplay"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "rodio"
    }
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
                if stop_rx.try_recv().is_ok() {
                    stop_playback_child(&mut child);
                    return Ok(());
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

fn analyze_transcript_jsonl(path: &Path) -> anyhow::Result<TranscriptSummary> {
    let reader = BufReader::new(File::open(path)?);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&line)?;
        if value.get("type").and_then(|v| v.as_str()) != Some("Results") {
            continue;
        }

        let text = value
            .pointer("/channel/alternatives/0/transcript")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if text.is_empty() {
            continue;
        }

        events.push(TranscriptEvent {
            start: value.get("start").and_then(|v| v.as_f64()).unwrap_or(0.0),
            channel: value
                .get("channel_index")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_i64()),
            is_final: value
                .get("is_final")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            text,
        });
    }

    let mut near_duplicates = 0usize;
    for (idx, event) in events.iter().enumerate() {
        if event.channel.is_none() {
            continue;
        }

        let duplicate = events[..idx].iter().rev().take(12).any(|previous| {
            previous.channel != event.channel
                && (event.start - previous.start).abs() <= 3.0
                && shares_phrase(&event.text, &previous.text)
        });

        if duplicate {
            near_duplicates += 1;
        }
    }

    Ok(TranscriptSummary {
        path: path_string(path),
        total_results: events.len(),
        final_results: events.iter().filter(|event| event.is_final).count(),
        channel_0_results: events
            .iter()
            .filter(|event| event.channel == Some(0))
            .count(),
        channel_1_results: events
            .iter()
            .filter(|event| event.channel == Some(1))
            .count(),
        near_duplicate_cross_channel_results: near_duplicates,
    })
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

fn lag_ms(lag_samples: isize, sample_rate: u32) -> f32 {
    lag_samples as f32 / sample_rate as f32 * 1000.0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cross_channel_duplicate_phrase() {
        assert!(shares_phrase(
            "queries yielding a statistic portrait",
            "of queries yielding a statistic",
        ));
    }

    #[test]
    fn ignores_distinct_transcript_phrases() {
        assert!(!shares_phrase(
            "the launch checklist is ready",
            "the meeting starts tomorrow",
        ));
    }

    #[test]
    fn correlation_tracks_delayed_speaker_echo() {
        let speaker = (0..1600)
            .map(|idx| if idx % 97 == 0 { 1.0 } else { 0.1 })
            .collect::<Vec<_>>();
        let mut mic = vec![0.0; speaker.len()];
        for idx in 240..speaker.len() {
            mic[idx] = speaker[idx - 240] * 0.3;
        }

        let correlation = best_abs_correlation(&mic, &speaker, 400).expect("correlation");

        assert!(correlation.value > 0.95);
        assert_eq!(correlation.lag_samples, 240);
    }
}
