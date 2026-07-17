use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use hypr_audio::AudioProvider;

use crate::{ListenerRuntime, TranscriptionMode};

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
    #[serde(default)]
    pub transcription_mode: TranscriptionMode,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub participant_human_ids: Vec<String>,
    #[serde(default)]
    pub self_human_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SessionConfigUpdate {
    pub session_id: String,
    pub languages: Vec<hypr_language::Language>,
    #[serde(default)]
    pub participant_human_ids: Vec<String>,
    #[serde(default)]
    pub self_human_id: Option<String>,
}

impl SessionParams {
    pub fn effective_transcription_mode(&self) -> TranscriptionMode {
        if self.transcription_mode == TranscriptionMode::Batch {
            return TranscriptionMode::Batch;
        }

        if let Some(model) =
            hypr_transcribe_soniqo::local_model_from_request(&self.base_url, &self.model)
        {
            return if model.supports_live_on_current_platform()
                && model.supports_languages(&self.languages)
            {
                TranscriptionMode::Live
            } else {
                TranscriptionMode::Batch
            };
        }

        if hypr_transcribe_soniqo::is_local_base_url(&self.base_url) {
            return TranscriptionMode::Batch;
        }

        TranscriptionMode::Live
    }

    pub fn uses_local_soniqo_live_model(&self) -> bool {
        hypr_transcribe_soniqo::local_model_from_request(&self.base_url, &self.model)
            .is_some_and(|model| model.supports_live())
    }
}

#[derive(Clone)]
pub struct SessionContext {
    pub runtime: Arc<dyn ListenerRuntime>,
    pub audio: Arc<dyn AudioProvider>,
    pub requested_transcription_mode: TranscriptionMode,
    pub params: SessionParams,
    pub app_dir: PathBuf,
    pub started_at_instant: Instant,
    pub started_at_system: SystemTime,
}

pub fn session_supervisor_name(session_id: &str) -> String {
    format!("{}{}", SESSION_SUPERVISOR_PREFIX, session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_params(base_url: &str, model: &str, mode: TranscriptionMode) -> SessionParams {
        SessionParams {
            session_id: "session".to_string(),
            languages: vec![],
            onboarding: false,
            transcription_mode: mode,
            model: model.to_string(),
            base_url: base_url.to_string(),
            api_key: String::new(),
            keywords: vec![],
            participant_human_ids: vec![],
            self_human_id: None,
        }
    }

    #[test]
    fn effective_mode_keeps_explicit_batch() {
        let params = session_params(
            hypr_transcribe_soniqo::LOCAL_BASE_URL,
            "soniqo-parakeet-streaming",
            TranscriptionMode::Batch,
        );

        assert_eq!(
            params.effective_transcription_mode(),
            TranscriptionMode::Batch
        );
    }

    #[test]
    fn effective_mode_forces_soniqo_batch_models_to_batch() {
        let params = session_params(
            hypr_transcribe_soniqo::LOCAL_BASE_URL,
            "soniqo-parakeet-batch",
            TranscriptionMode::Live,
        );

        assert_eq!(
            params.effective_transcription_mode(),
            TranscriptionMode::Batch
        );
    }

    #[test]
    fn effective_mode_detects_soniqo_loopback_base_url() {
        let params = session_params(
            "http://localhost:50060/v1",
            "soniqo-parakeet-streaming",
            TranscriptionMode::Live,
        );
        let expected = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            TranscriptionMode::Live
        } else {
            TranscriptionMode::Batch
        };

        assert_eq!(params.effective_transcription_mode(), expected);
    }

    #[test]
    fn effective_mode_rejects_soniqo_live_for_unsupported_language() {
        let mut params = session_params(
            hypr_transcribe_soniqo::LOCAL_BASE_URL,
            "soniqo-parakeet-streaming",
            TranscriptionMode::Live,
        );
        params.languages = vec![hypr_language::ISO639::Ko.into()];

        assert_eq!(
            params.effective_transcription_mode(),
            TranscriptionMode::Batch
        );
    }

    #[test]
    fn detects_local_soniqo_live_model() {
        let params = session_params(
            hypr_transcribe_soniqo::LOCAL_BASE_URL,
            "soniqo-parakeet-streaming",
            TranscriptionMode::Live,
        );

        assert!(params.uses_local_soniqo_live_model());
    }

    #[test]
    fn rejects_soniqo_batch_model_as_live_model() {
        let params = session_params(
            hypr_transcribe_soniqo::LOCAL_BASE_URL,
            "soniqo-parakeet-batch",
            TranscriptionMode::Live,
        );

        assert!(!params.uses_local_soniqo_live_model());
    }

    #[test]
    fn effective_mode_defaults_invalid_soniqo_model_to_batch() {
        let params = session_params(
            hypr_transcribe_soniqo::LOCAL_BASE_URL,
            "missing-model",
            TranscriptionMode::Live,
        );

        assert_eq!(
            params.effective_transcription_mode(),
            TranscriptionMode::Batch
        );
    }

    #[test]
    fn effective_mode_keeps_non_soniqo_live() {
        let params = session_params(
            "https://api.deepgram.com/v1",
            "nova-3-general",
            TranscriptionMode::Live,
        );

        assert_eq!(
            params.effective_transcription_mode(),
            TranscriptionMode::Live
        );
    }

    #[test]
    fn deserializes_missing_transcription_mode_as_live() {
        let value = serde_json::json!({
            "session_id": "session",
            "languages": [],
            "onboarding": false,
            "model": "nova-3-general",
            "base_url": "https://api.deepgram.com/v1",
            "api_key": "test-key",
            "keywords": [],
        });

        let params: SessionParams = serde_json::from_value(value).unwrap();

        assert_eq!(params.transcription_mode, TranscriptionMode::Live);
    }
}
