use owhisper_interface::stream::StreamResponse;

use crate::DegradedError;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "tauri-event", derive(tauri_specta::Event))]
#[serde(tag = "type")]
pub enum SessionLifecycleEvent {
    #[serde(rename = "inactive")]
    Inactive {
        session_id: String,
        error: Option<String>,
    },
    #[serde(rename = "active")]
    Active {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(rename = "stream_response")]
    StreamResponse {
        session_id: String,
        response: Box<StreamResponse>,
    },
}
