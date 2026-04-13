mod accumulator;
mod progressive;
mod simple;

use std::sync::Arc;

use owhisper_client::{AdapterKind, OpenAIAdapter};

use crate::{BatchEvent, BatchRuntime};

use progressive::run_progressive_batch_session;
use simple::run_direct_batch_for_adapter_kind;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum::Display, strum::EnumString)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum BatchProvider {
    Argmax,
    #[serde(rename = "whispercpp")]
    #[strum(serialize = "whispercpp")]
    WhisperLocal,
    Deepgram,
    Soniox,
    AssemblyAI,
    Fireworks,
    OpenAI,
    Gladia,
    ElevenLabs,
    Pyannote,
    DashScope,
    Mistral,
    Hyprnote,
    Am,
    Cactus,
    AquaVoice,
}

impl BatchProvider {
    pub fn to_adapter_kind(&self) -> Option<AdapterKind> {
        match self {
            Self::Argmax => Some(AdapterKind::Argmax),
            Self::Deepgram => Some(AdapterKind::Deepgram),
            Self::Soniox => Some(AdapterKind::Soniox),
            Self::AssemblyAI => Some(AdapterKind::AssemblyAI),
            Self::Fireworks => Some(AdapterKind::Fireworks),
            Self::OpenAI => Some(AdapterKind::OpenAI),
            Self::Gladia => Some(AdapterKind::Gladia),
            Self::ElevenLabs => Some(AdapterKind::ElevenLabs),
            Self::Pyannote => Some(AdapterKind::Pyannote),
            Self::Mistral => Some(AdapterKind::Mistral),
            Self::Hyprnote => Some(AdapterKind::Hyprnote),
            Self::AquaVoice => Some(AdapterKind::AquaVoice),
            Self::Am | Self::WhisperLocal | Self::Cactus | Self::DashScope => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct BatchParams {
    pub session_id: String,
    pub provider: BatchProvider,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum BatchRunMode {
    Direct,
    Streamed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct BatchRunOutput {
    pub session_id: String,
    pub mode: BatchRunMode,
    pub response: owhisper_interface::batch::Response,
}

pub async fn run_batch(
    runtime: Arc<dyn BatchRuntime>,
    params: BatchParams,
) -> crate::Result<BatchRunOutput> {
    runtime.emit(BatchEvent::BatchStarted {
        session_id: params.session_id.clone(),
    });

    let session_id = params.session_id.clone();
    let result = run_batch_inner(runtime.clone(), params).await;

    if let Err(error) = &result {
        let (code, message) = match error {
            crate::Error::BatchFailed(failure) => (failure.code(), failure.to_string()),
            _ => (crate::BatchErrorCode::Unknown, error.to_string()),
        };

        runtime.emit(BatchEvent::BatchFailed {
            session_id,
            code,
            error: message,
        });
    } else {
        let output = result.as_ref().unwrap();

        runtime.emit(BatchEvent::BatchResponse {
            session_id: output.session_id.clone(),
            response: output.response.clone(),
            mode: output.mode,
        });
        runtime.emit(BatchEvent::BatchCompleted {
            session_id: output.session_id.clone(),
        });
    }

    result
}

async fn run_batch_inner(
    runtime: Arc<dyn BatchRuntime>,
    params: BatchParams,
) -> crate::Result<BatchRunOutput> {
    let metadata_joined = tokio::task::spawn_blocking({
        let path = params.file_path.clone();
        move || hypr_audio_utils::audio_file_metadata(path)
    })
    .await;

    let metadata_result = match metadata_joined {
        Ok(result) => result,
        Err(err) => {
            let raw_error = format!("{err:?}");
            tracing::error!(error = %raw_error, "audio_metadata_task_join_failed");
            return Err(crate::BatchFailure::AudioMetadataJoinFailed.into());
        }
    };

    let metadata = match metadata_result {
        Ok(metadata) => metadata,
        Err(err) => {
            let raw_error = err.to_string();
            let message = format_user_friendly_error(&raw_error);
            tracing::error!(
                error = %raw_error,
                hyprnote.error.user_message = %message,
                "failed_to_read_audio_metadata"
            );
            return Err(crate::BatchFailure::AudioMetadataReadFailed { message }.into());
        }
    };

    let listen_params = build_listen_params(&params, metadata.channels, metadata.sample_rate);

    match params.provider {
        BatchProvider::Am => {
            let adapter_kind = resolve_batch_adapter_kind(&params, &listen_params);
            if supports_progressive_batch(adapter_kind, listen_params.model.as_deref()) {
                run_progressive_batch_session(runtime, params, listen_params).await
            } else {
                run_direct_batch_for_adapter_kind(adapter_kind, params, listen_params).await
            }
        }
        BatchProvider::WhisperLocal | BatchProvider::Cactus => {
            run_progressive_batch_session(runtime, params, listen_params).await
        }
        BatchProvider::OpenAI => {
            if OpenAIAdapter::supports_progressive_batch_model(listen_params.model.as_deref()) {
                run_progressive_batch_session(runtime, params, listen_params).await
            } else {
                run_direct_batch_for_adapter_kind(AdapterKind::OpenAI, params, listen_params).await
            }
        }
        BatchProvider::DashScope => Err(crate::BatchFailure::BatchCapabilityUnsupported {
            provider: batch_provider_label(BatchProvider::DashScope),
        }
        .into()),
        ref provider => {
            let adapter_kind = provider
                .to_adapter_kind()
                .expect("all non-special BatchProvider variants have an AdapterKind mapping");
            run_direct_batch_for_adapter_kind(adapter_kind, params, listen_params).await
        }
    }
}

fn resolve_batch_adapter_kind(
    params: &BatchParams,
    listen_params: &owhisper_interface::ListenParams,
) -> AdapterKind {
    AdapterKind::from_url_and_languages(
        &params.base_url,
        &listen_params.languages,
        listen_params.model.as_deref(),
    )
}

fn supports_progressive_batch(adapter_kind: AdapterKind, model: Option<&str>) -> bool {
    match adapter_kind {
        AdapterKind::Argmax | AdapterKind::Cactus => true,
        AdapterKind::OpenAI => OpenAIAdapter::supports_progressive_batch_model(model),
        _ => false,
    }
}

fn build_listen_params(
    params: &BatchParams,
    channels: u8,
    sample_rate: u32,
) -> owhisper_interface::ListenParams {
    owhisper_interface::ListenParams {
        model: params.model.clone(),
        channels,
        sample_rate,
        languages: params.languages.clone(),
        keywords: params.keywords.clone(),
        num_speakers: params.num_speakers,
        min_speakers: params.min_speakers,
        max_speakers: params.max_speakers,
        custom_query: None,
    }
}

pub(super) fn batch_provider_label(provider: BatchProvider) -> String {
    provider.to_string()
}


pub(super) fn session_span(session_id: &str) -> tracing::Span {
    tracing::info_span!("session", hyprnote.session.id = %session_id)
}

pub(super) fn format_user_friendly_error(error: &str) -> String {
    let error_lower = error.to_lowercase();

    if error_lower.contains("401") || error_lower.contains("unauthorized") {
        return "Authentication failed. Please check your API key in settings.".to_string();
    }
    if error_lower.contains("403") || error_lower.contains("forbidden") {
        return "Access denied. Your API key may not have permission for this operation."
            .to_string();
    }
    if error_lower.contains("429") || error_lower.contains("rate limit") {
        return "Rate limit exceeded. Please wait a moment and try again.".to_string();
    }
    if error_lower.contains("timeout") {
        return "Connection timed out. Please check your internet connection and try again."
            .to_string();
    }
    if error_lower.contains("connection refused")
        || error_lower.contains("failed to connect")
        || error_lower.contains("network")
    {
        return "Could not connect to the transcription service. Please check your internet connection.".to_string();
    }
    if error_lower.contains("invalid audio")
        || error_lower.contains("unsupported format")
        || error_lower.contains("codec")
    {
        return "The audio file format is not supported. Please try a different file.".to_string();
    }
    if error_lower.contains("file not found") || error_lower.contains("no such file") {
        return "Audio file not found. The recording may have been moved or deleted.".to_string();
    }

    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listen_params(model: Option<&str>) -> owhisper_interface::ListenParams {
        owhisper_interface::ListenParams {
            model: model.map(ToOwned::to_owned),
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        }
    }

    fn batch_params(provider: BatchProvider, base_url: &str) -> BatchParams {
        BatchParams {
            session_id: "session".to_string(),
            provider,
            file_path: "/tmp/audio.wav".to_string(),
            model: None,
            base_url: base_url.to_string(),
            api_key: "key".to_string(),
            languages: vec![hypr_language::ISO639::En.into()],
            keywords: vec![],
            num_speakers: None,
            min_speakers: None,
            max_speakers: None,
        }
    }

    #[test]
    fn build_listen_params_preserves_num_speakers() {
        let mut params = batch_params(BatchProvider::Pyannote, "https://api.pyannote.ai");
        params.num_speakers = Some(3);

        let listen_params = build_listen_params(&params, 2, 48_000);

        assert_eq!(listen_params.num_speakers, Some(3));
        assert_eq!(listen_params.channels, 2);
        assert_eq!(listen_params.sample_rate, 48_000);
    }

    #[test]
    fn build_listen_params_preserves_speaker_range_options() {
        let mut params = batch_params(BatchProvider::Pyannote, "https://api.pyannote.ai");
        params.min_speakers = Some(2);
        params.max_speakers = Some(4);

        let listen_params = build_listen_params(&params, 1, 16_000);
        assert_eq!(listen_params.min_speakers, Some(2));
        assert_eq!(listen_params.max_speakers, Some(4));
        assert!(listen_params.custom_query.is_none());
    }

    #[test]
    fn am_routes_pyannote_to_direct_batch() {
        let params = batch_params(BatchProvider::Am, "https://api.pyannote.ai");
        let adapter_kind = resolve_batch_adapter_kind(&params, &listen_params(None));

        assert_eq!(adapter_kind, AdapterKind::Pyannote);
        assert!(!supports_progressive_batch(adapter_kind, None));
    }

    #[test]
    fn am_routes_deepgram_to_direct_batch() {
        let params = batch_params(BatchProvider::Am, "https://api.deepgram.com/v1");
        let adapter_kind = resolve_batch_adapter_kind(&params, &listen_params(None));

        assert_eq!(adapter_kind, AdapterKind::Deepgram);
        assert!(!supports_progressive_batch(adapter_kind, None));
    }

    #[test]
    fn am_routes_local_argmax_to_progressive_batch() {
        let params = batch_params(BatchProvider::Am, "http://localhost:50060/v1");
        let adapter_kind = resolve_batch_adapter_kind(&params, &listen_params(None));

        assert_eq!(adapter_kind, AdapterKind::Argmax);
        assert!(supports_progressive_batch(adapter_kind, None));
    }

    #[test]
    fn am_routes_cactus_model_to_progressive_batch() {
        let params = batch_params(BatchProvider::Am, "http://localhost:50060/v1");
        let adapter_kind =
            resolve_batch_adapter_kind(&params, &listen_params(Some("cactus-whisper-small-int4")));

        assert_eq!(adapter_kind, AdapterKind::Cactus);
        assert!(supports_progressive_batch(
            adapter_kind,
            Some("cactus-whisper-small-int4"),
        ));
    }

    #[test]
    fn am_routes_openai_gpt_batch_to_progressive_batch() {
        let params = batch_params(BatchProvider::Am, "https://api.openai.com/v1");
        let adapter_kind =
            resolve_batch_adapter_kind(&params, &listen_params(Some("gpt-4o-transcribe")));

        assert_eq!(adapter_kind, AdapterKind::OpenAI);
        assert!(supports_progressive_batch(
            adapter_kind,
            Some("gpt-4o-transcribe"),
        ));
    }

    #[test]
    fn am_routes_openai_diarized_batch_to_direct_batch() {
        let params = batch_params(BatchProvider::Am, "https://api.openai.com/v1");
        let adapter_kind =
            resolve_batch_adapter_kind(&params, &listen_params(Some("gpt-4o-transcribe-diarize")));

        assert_eq!(adapter_kind, AdapterKind::OpenAI);
        assert!(!supports_progressive_batch(
            adapter_kind,
            Some("gpt-4o-transcribe-diarize"),
        ));
    }
}
