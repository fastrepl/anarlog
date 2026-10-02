use anlg_transcription_core::{listener, listener2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CaptureState {
    Active,
    Finalizing,
    Inactive,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSnapshot {
    pub state: CaptureState,
    pub active_session_id: Option<String>,
    pub finalizing_session_ids: Vec<String>,
    pub requested_live_transcription: Option<bool>,
    pub live_transcription_active: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_segments_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_segments: Option<Vec<listener::LiveTranscriptSegment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic_muted: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded: Option<listener::DegradedError>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct LiveTranscriptTarget {
    pub transcript_id: String,
    pub owner_user_id: String,
    pub created_at: String,
    pub started_at_ms: i64,
    pub memo: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct LiveTranscriptPersistence {
    pub session_id: String,
    pub transcript_id: String,
    pub transcript_created: bool,
    pub persisted_through_ms: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct StoppedCapture {
    pub session_id: String,
    pub stopped_at_ms: i64,
    pub duration_seconds: f64,
    pub chunked_audio: bool,
    pub audio_path: Option<String>,
    pub requested_live_transcription: bool,
    pub live_transcription_active: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CaptureRecovery {
    pub session_id: String,
    pub process_stopped: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub struct CaptureAudioGap {
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub struct CaptureAudioGaps {
    pub capture_started_at_ms: i64,
    pub gaps: Vec<CaptureAudioGap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_gap_started_at_ms: Option<i64>,
    pub awaiting_connection: bool,
    pub storage_failed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_through_ms: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CaptureParams {
    pub session_id: String,
    #[serde(default)]
    pub live_transcript: Option<LiveTranscriptTarget>,
    #[serde(default)]
    pub retain_audio: Option<bool>,
    pub languages: Vec<anlg_language::Language>,
    pub onboarding: bool,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub mic_device: Option<String>,
    #[serde(default)]
    pub transcription_mode: Option<listener::TranscriptionMode>,
    #[serde(default)]
    pub participant_human_ids: Vec<String>,
    #[serde(default)]
    pub self_human_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CaptureConfigUpdate {
    pub session_id: String,
    pub languages: Vec<anlg_language::Language>,
    #[serde(default)]
    pub participant_human_ids: Vec<String>,
    #[serde(default)]
    pub self_human_id: Option<String>,
    #[serde(default)]
    pub speaker_assignments: Vec<anlg_transcript::IdentityAssignment>,
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
        stopped_at_ms: i64,
        #[serde(default)]
        chunked_audio: bool,
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
    #[serde(rename = "mic_isolated")]
    MicIsolated { session_id: String, value: bool },
    #[serde(rename = "mic_dropouts")]
    MicDropouts { session_id: String, ratio: f32 },
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

#[derive(serde::Serialize, Clone, specta::Type, tauri_specta::Event)]
pub struct LiveTranscriptPersistenceEvent {
    pub status: LiveTranscriptPersistence,
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
    pub languages: Vec<anlg_language::Language>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub num_speakers: Option<u32>,
    #[serde(default)]
    pub min_speakers: Option<u32>,
    #[serde(default)]
    pub max_speakers: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_context: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TranscriptionSession {
    pub session_id: String,
    pub file_path: String,
    pub provider: Option<TranscriptionProvider>,
    pub model: Option<String>,
    pub started_at_ms: i64,
    pub resume_context: Option<String>,
    pub completed: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CompletedTranscription {
    pub session_id: String,
    pub response: owhisper_interface::batch::Response,
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
        // Pass the requested mode only; the listener root resolves the
        // effective mode through the shared policy in listener-core.
        let transcription_mode = value
            .transcription_mode
            .unwrap_or(listener::TranscriptionMode::Live);

        Self {
            session_id: value.session_id,
            retain_audio: value.retain_audio,
            languages: value.languages,
            onboarding: value.onboarding,
            transcription_mode,
            model: value.model,
            base_url: value.base_url,
            api_key: value.api_key,
            keywords: value.keywords,
            mic_device: value.mic_device,
            participant_human_ids: value.participant_human_ids,
            self_human_id: value.self_human_id,
            // The desktop config sync pushes the transcript's persisted hints
            // once the capture is active; a new transcript has none yet.
            speaker_assignments: Vec::new(),
        }
    }
}

impl From<CaptureConfigUpdate> for listener::actors::SessionConfigUpdate {
    fn from(value: CaptureConfigUpdate) -> Self {
        Self {
            session_id: value.session_id,
            languages: value.languages,
            participant_human_ids: value.participant_human_ids,
            self_human_id: value.self_human_id,
            speaker_assignments: value.speaker_assignments,
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

impl From<listener::Snapshot> for CaptureSnapshot {
    fn from(value: listener::Snapshot) -> Self {
        Self {
            state: CaptureState::from(value.state),
            active_session_id: value.active_session_id,
            finalizing_session_ids: value.finalizing_session_ids,
            requested_live_transcription: None,
            live_transcription_active: None,
            live_segments_session_id: None,
            live_segments: None,
            started_at_ms: None,
            mic_muted: None,
            degraded: None,
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
            listener::SessionDataEvent::MicIsolated { session_id, value } => {
                Self::MicIsolated { session_id, value }
            }
            listener::SessionDataEvent::MicDropouts { session_id, ratio } => {
                Self::MicDropouts { session_id, ratio }
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
            known_speakers: vec![],
        }
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

#[cfg(test)]
mod tests {
    use super::CaptureParams;
    use anlg_transcription_core::listener::{TranscriptionMode, actors::SessionParams};

    fn capture_params(transcription_mode: Option<TranscriptionMode>) -> CaptureParams {
        CaptureParams {
            session_id: "session-1".to_string(),
            live_transcript: None,
            retain_audio: None,
            languages: vec![],
            onboarding: false,
            model: "nova-3-general".to_string(),
            base_url: "https://api.deepgram.com/v1".to_string(),
            api_key: "test-key".to_string(),
            keywords: vec![],
            mic_device: Some("External Microphone".to_string()),
            transcription_mode,
            participant_human_ids: vec![],
            self_human_id: None,
        }
    }

    #[test]
    fn session_params_carry_selected_microphone_and_requested_mode() {
        for (requested, expected) in [
            (None, TranscriptionMode::Live),
            (Some(TranscriptionMode::Batch), TranscriptionMode::Batch),
        ] {
            let session: SessionParams = capture_params(requested).into();

            assert_eq!(session.transcription_mode, expected);
            assert_eq!(session.mic_device.as_deref(), Some("External Microphone"));
        }
    }
}
