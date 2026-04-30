use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::audio_capture::LiveCaptureSession;

#[cfg(target_os = "macos")]
use swift_rs::{Bool, Int, SRData, SRString, swift};

#[cfg(target_os = "macos")]
swift!(fn _speech_model_cache_dir(model_id: &SRString) -> SRString);
#[cfg(target_os = "macos")]
swift!(fn _speech_model_download_state(model_id: &SRString) -> SRString);
#[cfg(target_os = "macos")]
swift!(fn _speech_model_start_download(model_id: &SRString) -> Bool);
#[cfg(target_os = "macos")]
swift!(fn _speech_model_reset(model_id: &SRString) -> Bool);
#[cfg(target_os = "macos")]
swift!(fn _speech_transcribe_audio_file(
    model_id: &SRString,
    audio_path: &SRString,
    language: &SRString
) -> SRString);
#[cfg(target_os = "macos")]
swift!(fn _speech_diarize_audio_file(audio_path: &SRString, speaker_count: Int) -> SRString);
#[cfg(target_os = "macos")]
swift!(fn _speech_embed_speaker_audio_file(audio_path: &SRString, speakers_json: &SRString) -> SRString);
#[cfg(target_os = "macos")]
swift!(fn _speech_live_transcription_start(
    mode: &SRString,
    model_id: &SRString,
    recording_path: &SRString,
    language: &SRString
) -> SRString);
#[cfg(target_os = "macos")]
swift!(fn _speech_live_transcription_append(
    mixed_samples: &SRData,
    microphone_samples: &SRData,
    system_samples: &SRData
) -> SRString);
#[cfg(target_os = "macos")]
swift!(fn _speech_live_transcription_state() -> SRString);
#[cfg(target_os = "macos")]
swift!(fn _speech_live_transcription_request_stop() -> SRString);
#[cfg(target_os = "macos")]
swift!(fn _speech_live_transcription_stop() -> SRString);

#[derive(Default)]
pub struct TranscriptionManager {
    active: Option<CaptureHandle>,
}

struct CaptureHandle {
    capture: LiveCaptureSession,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTranscriptEntry {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTranscriptionState {
    pub running: bool,
    pub text: String,
    pub error: Option<String>,
    #[serde(default)]
    pub entries: Vec<LiveTranscriptEntry>,
    #[serde(default)]
    pub audio_path: String,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechModelDownloadState {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub current_file: Option<String>,
    #[serde(default)]
    pub progress_percent: Option<u8>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileTranscriptionPayload {
    #[serde(default)]
    text: String,
    error: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiarizationSegmentPayload {
    #[serde(default)]
    pub speaker: String,
    #[serde(default)]
    pub start_seconds: f64,
    #[serde(default)]
    pub end_seconds: f64,
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiarizationPayload {
    #[serde(default)]
    pub segments: Vec<DiarizationSegmentPayload>,
    #[serde(default)]
    pub speaker_count: usize,
    #[serde(default)]
    pub pipeline_source: String,
    error: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerEmbeddingRequest {
    #[serde(default)]
    pub speaker: String,
    #[serde(default)]
    pub segments: Vec<DiarizationSegmentPayload>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerEmbeddingSamplePayload {
    #[serde(default)]
    pub start_seconds: f64,
    #[serde(default)]
    pub end_seconds: f64,
    #[serde(default)]
    pub duration_seconds: f64,
    #[serde(default)]
    pub embedding: Vec<f32>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerEmbeddingPayload {
    #[serde(default)]
    pub speaker: String,
    #[serde(default)]
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub samples: Vec<SpeakerEmbeddingSamplePayload>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSpeakerEmbeddingPayload {
    #[serde(default)]
    pub speakers: Vec<SpeakerEmbeddingPayload>,
    error: Option<String>,
}

impl TranscriptionManager {
    pub fn start(
        &mut self,
        mode: &str,
        model_id: &str,
        recording_path: &Path,
        language: &str,
    ) -> Result<LiveTranscriptionState, String> {
        let _ = self.stop();
        info!(
            mode,
            model_id,
            recording_path = %recording_path.display(),
            "Starting speech-swift transcription session"
        );

        let initial = ensure_started(speech_live_transcription_start(
            mode,
            model_id,
            recording_path,
            language,
        )?)?;

        match CaptureHandle::start() {
            Ok(handle) => {
                self.active = Some(handle);
                Ok(initial)
            }
            Err(message) => {
                let _ = speech_live_transcription_stop();
                Err(message)
            }
        }
    }

    pub fn preload(&mut self, _model_id: &str) {}

    pub fn clear_preload(&mut self) {}

    pub fn request_stop(&mut self) -> Result<LiveTranscriptionState, String> {
        info!("Requesting speech-swift transcription shutdown");
        if let Some(active) = self.active.as_ref() {
            active.capture.request_stop()?;
            return speech_live_transcription_request_stop();
        }

        speech_live_transcription_state()
    }

    pub fn state(&mut self) -> Result<LiveTranscriptionState, String> {
        let Some(active) = self.active.as_ref() else {
            return speech_live_transcription_state();
        };

        if active.capture.is_running() {
            return speech_live_transcription_state();
        }

        let active = self
            .active
            .take()
            .ok_or_else(|| "Failed to access transcription state.".to_string())?;
        active.finish()
    }

    pub fn stop(&mut self) -> Result<LiveTranscriptionState, String> {
        let Some(active) = self.active.take() else {
            return speech_live_transcription_stop();
        };

        let _ = active.capture.request_stop();
        active.finish()
    }
}

impl CaptureHandle {
    fn start() -> Result<Self, String> {
        let capture = LiveCaptureSession::start(|mixed, microphone, system| {
            speech_live_transcription_append(&mixed, &microphone, &system)
        })?;
        Ok(Self { capture })
    }

    fn finish(self) -> Result<LiveTranscriptionState, String> {
        let capture_error = self.capture.take_error();
        self.capture.finish()?;

        let mut snapshot = speech_live_transcription_stop()?;
        if snapshot.error.is_none() {
            snapshot.error = capture_error;
        }

        Ok(snapshot)
    }
}

pub fn managed_model_path(model_id: &str) -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let model_id: SRString = model_id.into();
        let path = unsafe { _speech_model_cache_dir(&model_id) };
        Ok(PathBuf::from(path.as_str()))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("speech-swift is only available on macOS.".to_string())
    }
}

pub fn managed_model_download_state(model_id: &str) -> Result<SpeechModelDownloadState, String> {
    #[cfg(target_os = "macos")]
    {
        let model_id: SRString = model_id.into();
        decode_json(
            unsafe { _speech_model_download_state(&model_id) },
            "speech-swift model download state",
        )
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("speech-swift is only available on macOS.".to_string())
    }
}

pub fn start_managed_model_download(model_id: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let model_id: SRString = model_id.into();
        if unsafe { _speech_model_start_download(&model_id) } {
            return Ok(());
        }

        Err("Failed to start the speech-swift model download.".to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("speech-swift is only available on macOS.".to_string())
    }
}

pub fn reset_managed_model(model_id: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let model_id: SRString = model_id.into();
        if unsafe { _speech_model_reset(&model_id) } {
            return Ok(());
        }

        Err("Failed to reset the speech-swift model state.".to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("speech-swift is only available on macOS.".to_string())
    }
}

pub fn transcribe_audio_file(
    model_id: &str,
    audio_path: &Path,
    language: &str,
) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let model_id: SRString = model_id.into();
        let audio_path_string = audio_path.display().to_string();
        let audio_path: SRString = audio_path_string.as_str().into();
        let language: SRString = language.into();
        let response: FileTranscriptionPayload = decode_json(
            unsafe { _speech_transcribe_audio_file(&model_id, &audio_path, &language) },
            "speech-swift file transcription",
        )?;

        if let Some(error) = response
            .error
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            return Err(error.clone());
        }

        Ok(response.text)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = model_id;
        let _ = audio_path;
        let _ = language;
        Err("speech-swift is only available on macOS.".to_string())
    }
}

pub fn diarize_audio_file(
    audio_path: &Path,
    speaker_count: Option<usize>,
) -> Result<FileDiarizationPayload, String> {
    #[cfg(target_os = "macos")]
    {
        let audio_path_string = audio_path.display().to_string();
        let audio_path: SRString = audio_path_string.as_str().into();
        let speaker_count = speaker_count.unwrap_or(0) as Int;
        let response: FileDiarizationPayload = decode_json(
            unsafe { _speech_diarize_audio_file(&audio_path, speaker_count) },
            "speech-swift diarization",
        )?;

        if let Some(error) = response
            .error
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            return Err(error.clone());
        }

        Ok(response)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = audio_path;
        let _ = speaker_count;
        Err("speech-swift is only available on macOS.".to_string())
    }
}

pub fn embed_speaker_audio_file(
    audio_path: &Path,
    speakers: &[SpeakerEmbeddingRequest],
) -> Result<FileSpeakerEmbeddingPayload, String> {
    #[cfg(target_os = "macos")]
    {
        let audio_path_string = audio_path.display().to_string();
        let audio_path: SRString = audio_path_string.as_str().into();
        let speakers_json = serde_json::to_string(speakers)
            .map_err(|error| format!("Failed to encode speaker embedding request: {error}"))?;
        let speakers_json: SRString = speakers_json.as_str().into();
        let response: FileSpeakerEmbeddingPayload = decode_json(
            unsafe { _speech_embed_speaker_audio_file(&audio_path, &speakers_json) },
            "speech-swift speaker embeddings",
        )?;

        if let Some(error) = response
            .error
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            return Err(error.clone());
        }

        Ok(response)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = audio_path;
        let _ = speakers;
        Err("speech-swift is only available on macOS.".to_string())
    }
}

fn ensure_started(snapshot: LiveTranscriptionState) -> Result<LiveTranscriptionState, String> {
    if snapshot.running {
        return Ok(snapshot);
    }

    Err(snapshot
        .error
        .unwrap_or_else(|| "Failed to start the speech-swift transcription session.".to_string()))
}

fn speech_live_transcription_start(
    mode: &str,
    model_id: &str,
    recording_path: &Path,
    language: &str,
) -> Result<LiveTranscriptionState, String> {
    #[cfg(target_os = "macos")]
    {
        let mode: SRString = mode.into();
        let model_id: SRString = model_id.into();
        let recording_path_string = recording_path.display().to_string();
        let recording_path: SRString = recording_path_string.as_str().into();
        let language: SRString = language.into();
        decode_json(
            unsafe {
                _speech_live_transcription_start(&mode, &model_id, &recording_path, &language)
            },
            "speech-swift transcription state",
        )
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("speech-swift is only available on macOS.".to_string())
    }
}

fn speech_live_transcription_append(
    mixed_samples: &[f32],
    microphone_samples: &[f32],
    system_samples: &[f32],
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let encode = |samples: &[f32]| {
            let mut bytes = Vec::with_capacity(std::mem::size_of_val(samples));
            for sample in samples {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
            let data: SRData = bytes.as_slice().into();
            (bytes, data)
        };

        let (_mixed_bytes, mixed_data) = encode(mixed_samples);
        let (_microphone_bytes, microphone_data) = encode(microphone_samples);
        let (_system_bytes, system_data) = encode(system_samples);
        let message = unsafe {
            _speech_live_transcription_append(&mixed_data, &microphone_data, &system_data)
        };
        if message.as_str().is_empty() {
            return Ok(());
        }

        Err(message.as_str().to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = samples;
        Err("speech-swift is only available on macOS.".to_string())
    }
}

fn speech_live_transcription_state() -> Result<LiveTranscriptionState, String> {
    #[cfg(target_os = "macos")]
    {
        decode_json(
            unsafe { _speech_live_transcription_state() },
            "speech-swift transcription state",
        )
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("speech-swift is only available on macOS.".to_string())
    }
}

fn speech_live_transcription_request_stop() -> Result<LiveTranscriptionState, String> {
    #[cfg(target_os = "macos")]
    {
        decode_json(
            unsafe { _speech_live_transcription_request_stop() },
            "speech-swift transcription state",
        )
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("speech-swift is only available on macOS.".to_string())
    }
}

fn speech_live_transcription_stop() -> Result<LiveTranscriptionState, String> {
    #[cfg(target_os = "macos")]
    {
        decode_json(
            unsafe { _speech_live_transcription_stop() },
            "speech-swift transcription state",
        )
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("speech-swift is only available on macOS.".to_string())
    }
}

#[cfg(target_os = "macos")]
fn decode_json<T>(value: SRString, label: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(value.as_str())
        .map_err(|error| format!("Failed to decode {label}: {error}"))
}
