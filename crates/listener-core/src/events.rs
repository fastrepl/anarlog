use crate::{DegradedError, LiveTranscriptDelta, LiveTranscriptSegmentDelta, TranscriptionMode};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "tauri-event", derive(tauri_specta::Event))]
#[serde(tag = "type")]
pub enum SessionLifecycleEvent {
    #[serde(rename = "inactive")]
    Inactive {
        session_id: String,
        audio_path: Option<String>,
        error: Option<String>,
    },
    #[serde(rename = "active")]
    Active {
        session_id: String,
        #[serde(rename = "requestedTranscriptionMode")]
        requested_transcription_mode: TranscriptionMode,
        #[serde(rename = "currentTranscriptionMode")]
        current_transcription_mode: TranscriptionMode,
        error: Option<DegradedError>,
    },
    #[serde(rename = "finalizing")]
    Finalizing { session_id: String },
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "tauri-event", derive(tauri_specta::Event))]
#[serde(tag = "type")]
pub enum SessionProgressEvent {
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
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "tauri-event", derive(tauri_specta::Event))]
#[serde(tag = "type")]
pub enum SessionErrorEvent {
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

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "tauri-event", derive(tauri_specta::Event))]
#[serde(tag = "type")]
pub enum SessionDataEvent {
    #[serde(rename = "audio_amplitude")]
    AudioAmplitude {
        session_id: String,
        mic: u16,
        speaker: u16,
    },
    #[serde(rename = "mic_muted")]
    MicMuted { session_id: String, value: bool },
    /// Whether the microphone can only hear the local user: true when the default output is a
    /// headphone, so no far-end audio reaches the mic. Emitted each time capture (re)starts.
    #[serde(rename = "mic_isolated")]
    MicIsolated { session_id: String, value: bool },
    /// The mic delivered exact digital silence for `ratio` of the last few seconds. Bluetooth
    /// headsets in call mode gate their mic between words, and those words never reach the
    /// transcript. Emitted at most once per capture stream.
    #[serde(rename = "mic_dropouts")]
    MicDropouts { session_id: String, ratio: f32 },
    #[serde(rename = "transcript_delta")]
    TranscriptDelta {
        session_id: String,
        delta: Box<LiveTranscriptDelta>,
    },
    #[serde(rename = "transcript_segment_delta")]
    TranscriptSegmentDelta {
        session_id: String,
        delta: Box<LiveTranscriptSegmentDelta>,
    },
}
