mod accumulator;
mod actor;
mod bootstrap;

use std::sync::Arc;

use owhisper_client::{
    ArgmaxAdapter, AssemblyAIAdapter, BatchSttAdapter, DeepgramAdapter, ElevenLabsAdapter,
    FireworksAdapter, GladiaAdapter, HyprnoteAdapter, MistralAdapter, OpenAIAdapter, SonioxAdapter,
};
use tracing::Instrument;

use crate::{BatchEvent, BatchRuntime};

use actor::run_batch_streaming;

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
    DashScope,
    Mistral,
    Hyprnote,
    Am,
    Cactus,
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

    let listen_params = owhisper_interface::ListenParams {
        model: params.model.clone(),
        channels: metadata.channels,
        sample_rate: metadata.sample_rate,
        languages: params.languages.clone(),
        keywords: params.keywords.clone(),
        custom_query: None,
    };

    match params.provider {
        BatchProvider::Am | BatchProvider::WhisperLocal | BatchProvider::Cactus => {
            run_batch_streaming(runtime, params, listen_params).await
        }
        BatchProvider::Argmax => run_batch_simple::<ArgmaxAdapter>(params, listen_params).await,
        BatchProvider::Deepgram => run_batch_simple::<DeepgramAdapter>(params, listen_params).await,
        BatchProvider::Soniox => run_batch_simple::<SonioxAdapter>(params, listen_params).await,
        BatchProvider::AssemblyAI => {
            run_batch_simple::<AssemblyAIAdapter>(params, listen_params).await
        }
        BatchProvider::Fireworks => {
            run_batch_simple::<FireworksAdapter>(params, listen_params).await
        }
        BatchProvider::OpenAI => run_batch_simple::<OpenAIAdapter>(params, listen_params).await,
        BatchProvider::Gladia => run_batch_simple::<GladiaAdapter>(params, listen_params).await,
        BatchProvider::ElevenLabs => {
            run_batch_simple::<ElevenLabsAdapter>(params, listen_params).await
        }
        BatchProvider::DashScope => Err(crate::BatchFailure::ProviderRequestFailed {
            message: "DashScope does not support batch transcription".to_string(),
        }
        .into()),
        BatchProvider::Mistral => run_batch_simple::<MistralAdapter>(params, listen_params).await,
        BatchProvider::Hyprnote => run_batch_simple::<HyprnoteAdapter>(params, listen_params).await,
    }
}

async fn run_batch_simple<A: BatchSttAdapter>(
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    let span = session_span(&params.session_id);

    async {
        let client = owhisper_client::BatchClient::<A>::builder()
            .api_base(params.base_url.clone())
            .api_key(params.api_key.clone())
            .params(listen_params)
            .build();

        tracing::debug!("transcribing file: {}", params.file_path);
        let response = match client.transcribe_file(&params.file_path).await {
            Ok(response) => response,
            Err(err) => {
                let raw_error = format!("{err:?}");
                let message = format_user_friendly_error(&raw_error);
                tracing::error!(
                    error = %raw_error,
                    hyprnote.error.user_message = %message,
                    "batch transcription failed"
                );
                return Err(crate::BatchFailure::ProviderRequestFailed { message }.into());
            }
        };
        tracing::info!("batch transcription completed");

        Ok(BatchRunOutput {
            session_id: params.session_id,
            mode: BatchRunMode::Direct,
            response,
        })
    }
    .instrument(span)
    .await
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
