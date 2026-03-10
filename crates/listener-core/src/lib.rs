pub mod actors;
mod events;
mod runtime;

pub use events::*;
pub use runtime::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum State {
    Active,
    Inactive,
    Finalizing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TranscriptionMode {
    Live,
    Batch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum RecordingMode {
    Memory,
    Disk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum InMemoryRecordingDisposition {
    Discard,
    Persist,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StopSessionParams {
    pub in_memory_recording: Option<InMemoryRecordingDisposition>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum DegradedError {
    #[serde(rename = "authentication_failed")]
    AuthenticationFailed { provider: String },
    #[serde(rename = "upstream_unavailable")]
    UpstreamUnavailable { message: String },
    #[serde(rename = "connection_timeout")]
    ConnectionTimeout,
    #[serde(rename = "stream_error")]
    StreamError { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum StartSessionError {
    #[serde(rename = "session_already_running")]
    SessionAlreadyRunning,
    #[serde(rename = "failed_to_resolve_sessions_dir")]
    FailedToResolveSessionsDir,
    #[serde(rename = "failed_to_start_session")]
    FailedToStartSession,
}
