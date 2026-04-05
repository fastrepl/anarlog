use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use hypr_audio::AudioProvider;

use crate::{ListenerRuntime, RecordingMode, TranscriptionMode};

pub const SESSION_SUPERVISOR_PREFIX: &str = "session_supervisor_";

pub fn session_span(session_id: &str) -> tracing::Span {
    tracing::info_span!("session", hyprnote.session.id = %session_id)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SessionParams {
    pub session_id: String,
    pub languages: Vec<hypr_language::Language>,
    pub onboarding: bool,
    pub transcription_mode: TranscriptionMode,
    pub recording_mode: RecordingMode,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub participant_human_ids: Vec<String>,
    #[serde(default)]
    pub self_human_id: Option<String>,
}

#[derive(Clone)]
pub struct SessionContext {
    pub runtime: Arc<dyn ListenerRuntime>,
    pub audio: Arc<dyn AudioProvider>,
    pub params: SessionParams,
    pub app_dir: PathBuf,
    pub started_at_instant: Instant,
    pub started_at_system: SystemTime,
}

pub fn session_supervisor_name(session_id: &str) -> String {
    format!("{}{}", SESSION_SUPERVISOR_PREFIX, session_id)
}
