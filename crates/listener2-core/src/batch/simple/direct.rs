use std::path::{Path, PathBuf};
use std::time::Duration;

use owhisper_client::{
    AdapterKind, AnarlogAdapter, AquaVoiceAdapter, ArgmaxAdapter, AssemblyAIAdapter,
    AwsTranscribeAdapter, AzureSpeechAdapter, BatchSttAdapter, CartesiaAdapter, CohereAdapter,
    DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, GoogleCloudAdapter,
    GroqAdapter, MistralAdapter, OpenAIAdapter, OpenRouterAdapter, PyannoteAdapter, RevAiAdapter,
    SonioxAdapter, SpeechmaticsAdapter, TogetherAdapter, XaiAdapter,
};
use tracing::Instrument;

use anlg_audio_utils::Source;

use super::super::{
    BatchParams, BatchRunMode, BatchRunOutput, format_user_friendly_error, session_span,
};

pub(super) const DIRECT_BATCH_TIMEOUT_FLOOR: Duration = Duration::from_secs(15 * 60);
pub(super) const DIRECT_BATCH_TIMEOUT_CEILING: Duration = Duration::from_secs(6 * 60 * 60);
const DIRECT_BATCH_TIMEOUT_BUFFER: Duration = Duration::from_secs(5 * 60);
const DIRECT_BATCH_AUDIO_DURATION_MULTIPLIER: u32 = 2;
const ANARLOG_PROXY_MAX_AUDIO_BYTES: u64 = 512 * 1024 * 1024;

pub(super) enum PreparedBatchUpload {
    Original(PathBuf),
    Compressed {
        _temp_dir: tempfile::TempDir,
        path: PathBuf,
    },
}

impl PreparedBatchUpload {
    pub(super) fn path(&self) -> &Path {
        match self {
            Self::Original(path) | Self::Compressed { path, .. } => path,
        }
    }
}

macro_rules! dispatch_batch {
    ($ak:expr, $params:expr, $lp:expr,
     { $($var:ident => $adapter:ty),+ $(,)? },
     unsupported: [$($unsup:ident),* $(,)?]
    ) => {
        match $ak {
            $(AdapterKind::$var => {
                run_direct_batch::<$adapter>(&AdapterKind::$var.to_string(), $params, $lp).await
            })+
            $(AdapterKind::$unsup => {
                Err(crate::BatchFailure::DirectBatchUnsupported {
                    provider: AdapterKind::$unsup.to_string(),
                }.into())
            })*
        }
    };
}

pub(in crate::batch) async fn run_direct_batch_for_adapter_kind(
    adapter_kind: AdapterKind,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    if adapter_kind == AdapterKind::Anarlog {
        return run_anarlog_batch(params, listen_params).await;
    }

    dispatch_batch!(adapter_kind, params, listen_params, {
        Argmax => ArgmaxAdapter,
        Cartesia => CartesiaAdapter,
        Deepgram => DeepgramAdapter,
        Soniox => SonioxAdapter,
        AssemblyAI => AssemblyAIAdapter,
        Fireworks => FireworksAdapter,
        OpenAI => OpenAIAdapter,
        OpenRouter => OpenRouterAdapter,
        Gladia => GladiaAdapter,
        ElevenLabs => ElevenLabsAdapter,
        Pyannote => PyannoteAdapter,
        Mistral => MistralAdapter,
        Anarlog => AnarlogAdapter,
        AquaVoice => AquaVoiceAdapter,
        Cohere => CohereAdapter,
        AwsTranscribe => AwsTranscribeAdapter,
        AzureSpeech => AzureSpeechAdapter,
        GoogleCloud => GoogleCloudAdapter,
        Groq => GroqAdapter,
        RevAi => RevAiAdapter,
        Speechmatics => SpeechmaticsAdapter,
        Together => TogetherAdapter,
        Xai => XaiAdapter,
    }, unsupported: [DashScope])
}

async fn run_anarlog_batch(
    mut params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    let upload =
        prepare_anarlog_batch_upload(&params.file_path, ANARLOG_PROXY_MAX_AUDIO_BYTES).await?;
    params.file_path = upload.path().to_string_lossy().into_owned();
    run_direct_batch::<AnarlogAdapter>(&AdapterKind::Anarlog.to_string(), params, listen_params)
        .await
}

pub(super) async fn prepare_anarlog_batch_upload(
    file_path: &str,
    max_bytes: u64,
) -> crate::Result<PreparedBatchUpload> {
    let source_path = PathBuf::from(file_path);
    let source_size = tokio::fs::metadata(&source_path).await?.len();
    if source_size <= max_bytes {
        return Ok(PreparedBatchUpload::Original(source_path));
    }

    let is_wav = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"));
    if !is_wav {
        return Err(crate::BatchFailure::DirectRequestFailed {
            provider: AdapterKind::Anarlog.to_string(),
            message:
                "This recording is too large for cloud transcription. Convert it to MP3 and try again."
                    .to_string(),
        }
        .into());
    }

    let temp_dir = tempfile::tempdir().map_err(|error| {
        tracing::error!(%error, "large_batch_audio_temp_dir_failed");
        crate::BatchFailure::DirectRequestFailed {
            provider: AdapterKind::Anarlog.to_string(),
            message: "Anarlog couldn't prepare this large recording for transcription.".to_string(),
        }
    })?;
    let encoded_path = temp_dir.path().join("audio.mp3");
    let encode_source = source_path.clone();
    let encode_target = encoded_path.clone();
    tokio::task::spawn_blocking(move || anlg_mp3::encode_wav(&encode_source, &encode_target))
        .await
        .map_err(|error| {
            tracing::error!(%error, "large_batch_audio_encode_task_failed");
            crate::BatchFailure::DirectRequestFailed {
                provider: AdapterKind::Anarlog.to_string(),
                message: "Anarlog couldn't prepare this large recording for transcription."
                    .to_string(),
            }
        })?
        .map_err(|error| {
            tracing::error!(%error, "large_batch_audio_encode_failed");
            crate::BatchFailure::DirectRequestFailed {
                provider: AdapterKind::Anarlog.to_string(),
                message: "Anarlog couldn't prepare this large recording for transcription."
                    .to_string(),
            }
        })?;

    let encoded_size = tokio::fs::metadata(&encoded_path).await?.len();
    if encoded_size > max_bytes {
        return Err(crate::BatchFailure::DirectRequestFailed {
            provider: AdapterKind::Anarlog.to_string(),
            message:
                "This recording is too large for cloud transcription. Split it into smaller files and try again."
                    .to_string(),
        }
        .into());
    }

    tracing::info!(
        source_size,
        encoded_size,
        "large_batch_audio_compressed_for_upload"
    );

    Ok(PreparedBatchUpload::Compressed {
        _temp_dir: temp_dir,
        path: encoded_path,
    })
}

async fn run_direct_batch<A: BatchSttAdapter>(
    provider: &str,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    let timeout = direct_batch_timeout(&params.file_path);
    run_direct_batch_with_timeout::<A>(provider, params, listen_params, timeout).await
}

pub(super) async fn run_direct_batch_with_timeout<A: BatchSttAdapter>(
    provider: &str,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
    timeout: Duration,
) -> crate::Result<BatchRunOutput> {
    let span = session_span(&params.session_id);

    async {
        let client = owhisper_client::BatchClient::<A>::builder()
            .api_base(params.base_url.clone())
            .api_key(params.api_key.clone())
            .params(listen_params)
            .build();

        tracing::debug!("transcribing file: {}", params.file_path);
        let response =
            match tokio::time::timeout(timeout, client.transcribe_file(&params.file_path)).await {
                Ok(Ok(response)) => response,
                Ok(Err(err)) => {
                    let raw_error = format!("{err:?}");
                    let message = format_user_friendly_error(&raw_error);
                    tracing::error!(
                        error = %raw_error,
                        anarlog.error.user_message = %message,
                        "batch transcription failed"
                    );
                    return Err(crate::BatchFailure::DirectRequestFailed {
                        provider: provider.to_string(),
                        message,
                    }
                    .into());
                }
                Err(_) => {
                    tracing::error!(
                        timeout_seconds = timeout.as_secs(),
                        "batch transcription timed out"
                    );
                    return Err(crate::BatchFailure::DirectRequestTimedOut {
                        provider: provider.to_string(),
                        timeout_seconds: timeout.as_secs(),
                    }
                    .into());
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

fn direct_batch_timeout(file_path: &str) -> Duration {
    let audio_duration = anlg_audio_utils::source_from_path(file_path)
        .ok()
        .and_then(|source| source.total_duration());
    direct_batch_timeout_for_audio(audio_duration)
}

pub(super) fn direct_batch_timeout_for_audio(audio_duration: Option<Duration>) -> Duration {
    let timeout = audio_duration
        .map(|duration| {
            duration
                .saturating_mul(DIRECT_BATCH_AUDIO_DURATION_MULTIPLIER)
                .saturating_add(DIRECT_BATCH_TIMEOUT_BUFFER)
        })
        .unwrap_or(DIRECT_BATCH_TIMEOUT_FLOOR);

    timeout
        .max(DIRECT_BATCH_TIMEOUT_FLOOR)
        .min(DIRECT_BATCH_TIMEOUT_CEILING)
}
