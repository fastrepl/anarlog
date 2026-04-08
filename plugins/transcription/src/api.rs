use hypr_transcription_core::{listener, listener2};
use owhisper_client::AdapterKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CaptureState {
    Active,
    Finalizing,
    Inactive,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CaptureParams {
    pub session_id: String,
    pub languages: Vec<hypr_language::Language>,
    pub onboarding: bool,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub participant_human_ids: Vec<String>,
    #[serde(default)]
    pub self_human_id: Option<String>,
}

impl CaptureParams {
    fn default_transcription_mode(&self) -> listener::TranscriptionMode {
        let adapter_kind =
            AdapterKind::from_url_and_languages(&self.base_url, &[], Some(&self.model));

        if matches!(adapter_kind, AdapterKind::Argmax | AdapterKind::Cactus) {
            return listener::TranscriptionMode::Batch;
        }

        if adapter_kind.is_supported_languages_live(&[], Some(&self.model)) {
            listener::TranscriptionMode::Live
        } else {
            listener::TranscriptionMode::Batch
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event)]
#[serde(tag = "type")]
pub enum CaptureLifecycleEvent {
    #[serde(rename = "started")]
    Started {
        session_id: String,
        requested_live_transcription: bool,
        live_transcription_active: bool,
        degraded: Option<listener::DegradedError>,
    },
    #[serde(rename = "finalizing")]
    Finalizing { session_id: String },
    #[serde(rename = "stopped")]
    Stopped {
        session_id: String,
        audio_path: Option<String>,
        requested_live_transcription: bool,
        live_transcription_active: bool,
        error: Option<String>,
    },
}

#[derive(serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event)]
#[serde(tag = "type")]
pub enum CaptureStatusEvent {
    #[serde(rename = "audio_initializing")]
    AudioInitializing { session_id: String },
    #[serde(rename = "audio_ready")]
    AudioReady {
        session_id: String,
        device: Option<String>,
    },
    #[serde(rename = "connecting")]
    Connecting { session_id: String },
    #[serde(rename = "connected")]
    Connected { session_id: String, adapter: String },
    #[serde(rename = "audio_error")]
    AudioError {
        session_id: String,
        error: String,
        device: Option<String>,
        is_fatal: bool,
    },
    #[serde(rename = "connection_error")]
    ConnectionError { session_id: String, error: String },
}

#[derive(serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event)]
#[serde(tag = "type")]
pub enum CaptureDataEvent {
    #[serde(rename = "audio_amplitude")]
    AudioAmplitude {
        session_id: String,
        mic: u16,
        speaker: u16,
    },
    #[serde(rename = "mic_muted")]
    MicMuted { session_id: String, value: bool },
    #[serde(rename = "transcript_delta")]
    TranscriptDelta {
        session_id: String,
        delta: Box<listener::LiveTranscriptDelta>,
    },
    #[serde(rename = "transcript_segment_delta")]
    TranscriptSegmentDelta {
        session_id: String,
        delta: Box<listener::LiveTranscriptSegmentDelta>,
    },
}

pub type TranscriptionErrorCode = listener2::BatchErrorCode;
pub type TranscriptionFailure = listener2::BatchFailure;
pub type TranscriptionProvider = listener2::BatchProvider;
pub type TranscriptionRunMode = listener2::BatchRunMode;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TranscriptionParams {
    pub session_id: String,
    pub provider: TranscriptionProvider,
    pub file_path: String,
    #[serde(default)]
    pub model: Option<String>,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub languages: Vec<hypr_language::Language>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub num_speakers: Option<u32>,
    #[serde(default)]
    pub min_speakers: Option<u32>,
    #[serde(default)]
    pub max_speakers: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TranscriptionOutput {
    pub session_id: String,
    pub mode: TranscriptionRunMode,
    pub response: owhisper_interface::batch::Response,
}

#[derive(serde::Serialize, Clone, specta::Type, tauri_specta::Event)]
#[serde(tag = "type")]
pub enum TranscriptionEvent {
    #[serde(rename = "started")]
    Started { session_id: String },
    #[serde(rename = "progress")]
    Progress {
        session_id: String,
        event: owhisper_interface::batch_stream::BatchStreamEvent,
    },
    #[serde(rename = "completed")]
    Completed {
        session_id: String,
        response: owhisper_interface::batch::Response,
        mode: TranscriptionRunMode,
    },
    #[serde(rename = "stopped")]
    Stopped { session_id: String },
    #[serde(rename = "failed")]
    Failed {
        session_id: String,
        code: TranscriptionErrorCode,
        error: String,
    },
}

impl From<CaptureParams> for listener::actors::SessionParams {
    fn from(value: CaptureParams) -> Self {
        let transcription_mode = value.default_transcription_mode();

        Self {
            session_id: value.session_id,
            languages: value.languages,
            onboarding: value.onboarding,
            transcription_mode,
            model: value.model,
            base_url: value.base_url,
            api_key: value.api_key,
            keywords: value.keywords,
            participant_human_ids: value.participant_human_ids,
            self_human_id: value.self_human_id,
        }
    }
}

impl From<listener::State> for CaptureState {
    fn from(value: listener::State) -> Self {
        match value {
            listener::State::Active => Self::Active,
            listener::State::Finalizing => Self::Finalizing,
            listener::State::Inactive => Self::Inactive,
        }
    }
}

impl From<listener::SessionProgressEvent> for CaptureStatusEvent {
    fn from(value: listener::SessionProgressEvent) -> Self {
        match value {
            listener::SessionProgressEvent::AudioInitializing { session_id } => {
                Self::AudioInitializing { session_id }
            }
            listener::SessionProgressEvent::AudioReady { session_id, device } => {
                Self::AudioReady { session_id, device }
            }
            listener::SessionProgressEvent::Connecting { session_id } => {
                Self::Connecting { session_id }
            }
            listener::SessionProgressEvent::Connected {
                session_id,
                adapter,
            } => Self::Connected {
                session_id,
                adapter,
            },
        }
    }
}

impl From<listener::SessionErrorEvent> for CaptureStatusEvent {
    fn from(value: listener::SessionErrorEvent) -> Self {
        match value {
            listener::SessionErrorEvent::AudioError {
                session_id,
                error,
                device,
                is_fatal,
            } => Self::AudioError {
                session_id,
                error,
                device,
                is_fatal,
            },
            listener::SessionErrorEvent::ConnectionError { session_id, error } => {
                Self::ConnectionError { session_id, error }
            }
        }
    }
}

impl From<listener::SessionDataEvent> for CaptureDataEvent {
    fn from(value: listener::SessionDataEvent) -> Self {
        match value {
            listener::SessionDataEvent::AudioAmplitude {
                session_id,
                mic,
                speaker,
            } => Self::AudioAmplitude {
                session_id,
                mic,
                speaker,
            },
            listener::SessionDataEvent::MicMuted { session_id, value } => {
                Self::MicMuted { session_id, value }
            }
            listener::SessionDataEvent::TranscriptDelta { session_id, delta } => {
                Self::TranscriptDelta { session_id, delta }
            }
            listener::SessionDataEvent::TranscriptSegmentDelta { session_id, delta } => {
                Self::TranscriptSegmentDelta { session_id, delta }
            }
        }
    }
}

impl From<TranscriptionParams> for listener2::BatchParams {
    fn from(value: TranscriptionParams) -> Self {
        Self {
            session_id: value.session_id,
            provider: value.provider,
            file_path: value.file_path,
            model: value.model,
            base_url: value.base_url,
            api_key: value.api_key,
            languages: value.languages,
            keywords: value.keywords,
            num_speakers: value.num_speakers,
            min_speakers: value.min_speakers,
            max_speakers: value.max_speakers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureParams;
    use hypr_transcription_core::listener::TranscriptionMode;

    fn capture_params(base_url: &str, model: &str) -> CaptureParams {
        CaptureParams {
            session_id: "session-1".to_string(),
            languages: vec![],
            onboarding: false,
            model: model.to_string(),
            base_url: base_url.to_string(),
            api_key: "test-key".to_string(),
            keywords: vec![],
            participant_human_ids: vec![],
            self_human_id: None,
        }
    }

    #[test]
    fn defaults_realtime_provider_to_live_mode() {
        let params = capture_params("https://api.deepgram.com/v1", "nova-3-general");

        assert_eq!(params.default_transcription_mode(), TranscriptionMode::Live);
    }

    #[test]
    fn defaults_openai_capture_to_batch_mode() {
        let params = capture_params("https://api.openai.com/v1", "gpt-4o-transcribe");

        assert_eq!(
            params.default_transcription_mode(),
            TranscriptionMode::Batch
        );
    }

    #[test]
    fn defaults_pyannote_capture_to_batch_mode() {
        let params = capture_params("https://api.pyannote.ai", "parakeet-tdt-0.6b-v3");

        assert_eq!(
            params.default_transcription_mode(),
            TranscriptionMode::Batch
        );
    }

    #[test]
    fn defaults_local_cactus_capture_to_batch_mode() {
        let params = capture_params(
            "http://localhost:50060/v1",
            "cactus-parakeet-tdt-0.6b-v3-int8",
        );

        assert_eq!(
            params.default_transcription_mode(),
            TranscriptionMode::Batch
        );
    }
}

impl From<listener2::BatchRunOutput> for TranscriptionOutput {
    fn from(value: listener2::BatchRunOutput) -> Self {
        Self {
            session_id: value.session_id,
            mode: value.mode,
            response: value.response,
        }
    }
}

impl From<listener2::BatchEvent> for TranscriptionEvent {
    fn from(value: listener2::BatchEvent) -> Self {
        match value {
            listener2::BatchEvent::BatchStarted { session_id } => Self::Started { session_id },
            listener2::BatchEvent::BatchCompleted { .. } => {
                unreachable!("batch completed is represented by transcription completed")
            }
            listener2::BatchEvent::BatchResponse {
                session_id,
                response,
                mode,
            } => Self::Completed {
                session_id,
                response,
                mode,
            },
            listener2::BatchEvent::BatchResponseStreamed { session_id, event } => {
                Self::Progress { session_id, event }
            }
            listener2::BatchEvent::BatchFailed {
                session_id,
                code,
                error,
            } => Self::Failed {
                session_id,
                code,
                error,
            },
        }
    }
}
