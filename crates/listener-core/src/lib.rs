pub mod actors;
mod events;
mod live_transcript;
mod runtime;

pub use events::*;
pub use live_transcript::*;
pub use runtime::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum State {
    Active,
    Inactive,
    Finalizing,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub state: State,
    pub active_session_id: Option<String>,
    pub finalizing_session_ids: Vec<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TranscriptionMode {
    #[default]
    Live,
    Batch,
}

pub(crate) fn expected_speakers_per_channel(
    participant_human_ids: &[String],
    self_human_id: Option<&str>,
) -> Option<u32> {
    let mut remote_participants = participant_human_ids
        .iter()
        .filter(|participant| Some(participant.as_str()) != self_human_id)
        .cloned()
        .collect::<Vec<_>>();
    remote_participants.sort();
    remote_participants.dedup();

    let count = if remote_participants.is_empty() && self_human_id.is_some() {
        1
    } else {
        remote_participants.len()
    };

    u32::try_from(count).ok().filter(|count| *count > 0)
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
