//! Shipping's dictation recorder (`plugins/dictation/src/recorder.rs`): one owner records the
//! microphone into a temporary 16 kHz WAV, reporting its level and an optional live preview.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use anlg_audio::AudioProvider;
use desktop_runtime::{Result, ServiceError};
use futures::StreamExt;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::preview::{Preview, PreviewConfig};

pub const SAMPLE_RATE: u32 = 16_000;
const CHUNK_SIZE: usize = 1_600;
const MAX_RECORDING_SECONDS: u64 = 300;

#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    Amplitude(f32),
    Transcript { text: String, partial: String },
    PreviewUnavailable,
}

pub type Updates = Arc<dyn Fn(Update) + Send + Sync>;

#[derive(Debug)]
pub struct Recorded {
    pub path: PathBuf,
    pub duration_ms: u64,
    /// The live preview's final text, when it finished cleanly.
    pub transcript: Option<String>,
}

struct Active {
    owner: String,
    cancellation: CancellationToken,
    task: JoinHandle<Result<Recorded>>,
    finalize: Arc<AtomicBool>,
}

pub struct Recorder {
    runtime: tokio::runtime::Handle,
    active: Mutex<Option<Active>>,
    completed: Mutex<HashSet<PathBuf>>,
}

fn failed(message: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(message.to_string().into())
}

fn already_recording() -> ServiceError {
    failed("voice recording is already active")
}

impl Recorder {
    pub fn new(runtime: tokio::runtime::Handle) -> Self {
        Self {
            runtime,
            active: Mutex::new(None),
            completed: Mutex::new(HashSet::new()),
        }
    }

    /// Opens the microphone; blocks while the device starts.
    pub fn start(
        &self,
        audio: Arc<dyn AudioProvider>,
        microphone: Option<String>,
        owner: String,
        preview: Option<PreviewConfig>,
        updates: Option<Updates>,
    ) -> Result<()> {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.is_some() {
            return Err(already_recording());
        }
        // Capture streams spawn their reader on the current Tokio runtime.
        let _runtime = self.runtime.enter();
        let stream = audio
            .open_mic_capture(microphone, SAMPLE_RATE, CHUNK_SIZE)
            .map_err(|error| failed(format!("microphone capture failed: {error}")))?;
        let (_, path) = tempfile::Builder::new()
            .prefix("anarlog-dictation-")
            .suffix(".wav")
            .tempfile()
            .and_then(|file| file.keep().map_err(|error| error.error))
            .map_err(|error| failed(format!("voice recording failed: {error}")))?;
        let cancellation = CancellationToken::new();
        let finalize = Arc::new(AtomicBool::new(false));
        let task = {
            let cancellation = cancellation.clone();
            let finalize = finalize.clone();
            let handle = self.runtime.clone();
            self.runtime.spawn(async move {
                let preview = preview
                    .zip(updates.clone())
                    .map(|(config, updates)| Preview::start(&handle, config, updates));
                let result = record(stream, cancellation, &path, preview, updates, finalize).await;
                if result.is_err() {
                    let _ = std::fs::remove_file(&path);
                }
                result
            })
        };
        *active = Some(Active {
            owner,
            cancellation,
            task,
            finalize,
        });
        Ok(())
    }

    pub async fn stop(&self, owner: &str) -> Result<Recorded> {
        let active = self
            .take(owner)?
            .ok_or_else(|| failed("voice recording is not active"))?;
        active.finalize.store(true, Ordering::SeqCst);
        active.cancellation.cancel();
        let recorded = active
            .task
            .await
            .map_err(|error| failed(format!("voice recording task failed: {error}")))??;
        self.completed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(recorded.path.clone());
        Ok(recorded)
    }

    pub async fn cancel(&self, owner: &str) -> Result<()> {
        let Some(active) = self.take(owner)? else {
            return Ok(());
        };
        active.cancellation.cancel();
        match active.task.await {
            Ok(Ok(recorded)) => {
                let _ = std::fs::remove_file(recorded.path);
            }
            Ok(Err(_)) => {}
            Err(error) => return Err(failed(format!("voice recording task failed: {error}"))),
        }
        Ok(())
    }

    /// Deletes a finished recording; only files this recorder produced can be removed.
    pub fn discard(&self, path: &Path) -> Result<()> {
        if !self
            .completed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(path)
        {
            return Err(failed("unknown voice recording"));
        }
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(failed(format!("voice recording failed: {error}"))),
        }
    }

    fn take(&self, owner: &str) -> Result<Option<Active>> {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.as_ref().is_some_and(|active| active.owner != owner) {
            return Err(already_recording());
        }
        Ok(active.take())
    }
}

/// Microphone level as shipping reports it: frame RMS, scaled and clamped to 0..1.
pub fn amplitude(samples: &[f32]) -> f32 {
    let sum: f64 = samples.iter().map(|s| f64::from(*s).powi(2)).sum();
    let rms = (sum / samples.len().max(1) as f64).sqrt();
    (rms * 8.0).clamp(0.0, 1.0) as f32
}

async fn record(
    mut stream: anlg_audio::CaptureStream,
    cancellation: CancellationToken,
    path: &Path,
    mut preview: Option<Preview>,
    updates: Option<Updates>,
    finalize: Arc<AtomicBool>,
) -> Result<Recorded> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let recording = |error: hound::Error| failed(format!("voice recording failed: {error}"));
    let mut writer = hound::WavWriter::create(path, spec).map_err(recording)?;
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(MAX_RECORDING_SECONDS));
    tokio::pin!(deadline);
    let mut samples_written = 0_u64;
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => break,
            _ = &mut deadline => break,
            next = stream.next() => {
                let Some(frame) = next else { break };
                let frame = frame.map_err(|error| failed(format!("microphone capture failed: {error}")))?;
                let samples = frame.preferred_mic();
                for sample in samples.iter() {
                    writer.write_sample(*sample).map_err(recording)?;
                }
                samples_written += samples.len() as u64;
                if let Some(updates) = &updates {
                    updates(Update::Amplitude(amplitude(&samples)));
                    if preview.as_ref().is_some_and(|preview| !preview.send(&samples)) {
                        preview = None;
                    }
                }
            }
        }
    }
    writer.finalize().map_err(recording)?;
    let transcript = match preview {
        Some(preview) if finalize.load(Ordering::SeqCst) => preview.finish().await,
        _ => None,
    };
    Ok(Recorded {
        path: path.to_path_buf(),
        duration_ms: samples_written.saturating_mul(1_000) / u64::from(SAMPLE_RATE),
        transcript,
    })
}
