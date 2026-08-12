use std::path::{Path, PathBuf};
use std::time::Duration;

use owhisper_client::{
    AdapterKind, AnarlogAdapter, AquaVoiceAdapter, ArgmaxAdapter, AssemblyAIAdapter,
    AwsTranscribeAdapter, AzureSpeechAdapter, BatchSttAdapter, BatchUploadLimit, CartesiaAdapter,
    CohereAdapter, DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter,
    GoogleCloudAdapter, GroqAdapter, MistralAdapter, OpenAIAdapter, OpenRouterAdapter,
    PyannoteAdapter, RevAiAdapter, SonioxAdapter, SpeechmaticsAdapter, TogetherAdapter, XaiAdapter,
};
use owhisper_interface::batch::{Alternatives, Channel, Response, Results};
use tracing::Instrument;

use super::super::upload::{audio_duration, segment_plan, split_batch_upload};
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
    ($ak:expr, $params:expr, $lp:expr, $limit:expr,
     { $($var:ident => $adapter:ty),+ $(,)? },
     unsupported: [$($unsup:ident),* $(,)?]
    ) => {
        match $ak {
            $(AdapterKind::$var => {
                run_direct_batch::<$adapter>(&AdapterKind::$var.to_string(), $params, $lp, $limit)
                    .await
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

    let limit = adapter_kind.batch_upload_limit();

    dispatch_batch!(adapter_kind, params, listen_params, limit, {
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
    run_direct_batch::<AnarlogAdapter>(
        &AdapterKind::Anarlog.to_string(),
        params,
        listen_params,
        None,
    )
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

pub(super) async fn run_direct_batch<A: BatchSttAdapter>(
    provider: &str,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
    limit: Option<BatchUploadLimit>,
) -> crate::Result<BatchRunOutput> {
    let audio_duration = audio_duration(&params.file_path);
    let timeout = direct_batch_timeout_for_audio(audio_duration);

    match segment_plan(&params.file_path, audio_duration, limit) {
        Some(segment_duration) => {
            run_segmented_batch::<A>(provider, params, listen_params, segment_duration, timeout)
                .await
        }
        None => run_direct_batch_with_timeout::<A>(provider, params, listen_params, timeout).await,
    }
}

async fn run_segmented_batch<A: BatchSttAdapter>(
    provider: &str,
    params: BatchParams,
    mut listen_params: owhisper_interface::ListenParams,
    segment_duration: Duration,
    timeout: Duration,
) -> crate::Result<BatchRunOutput> {
    let segments = split_batch_upload(&params.file_path, segment_duration, provider).await?;
    listen_params.channels = 1;

    let mut responses = Vec::with_capacity(segments.paths().len());
    for path in segments.paths() {
        let mut segment_params = params.clone();
        segment_params.file_path = path.to_string_lossy().into_owned();

        let output = run_direct_batch_with_timeout::<A>(
            provider,
            segment_params,
            listen_params.clone(),
            timeout,
        )
        .await?;
        responses.push(output.response);
    }

    Ok(BatchRunOutput {
        session_id: params.session_id,
        mode: BatchRunMode::Direct,
        response: merge_segment_responses(responses, segment_duration),
    })
}

/// Segments are transcribed independently, so their timestamps restart at zero.
pub(super) fn merge_segment_responses(
    responses: Vec<Response>,
    segment_duration: Duration,
) -> Response {
    let mut metadata = serde_json::Value::Null;
    let mut speaker_labels = Vec::new();
    let mut speaker_segments = Vec::new();
    let mut speaker_offset = 0;
    let mut transcripts: Vec<String> = Vec::new();
    let mut words = Vec::new();

    for (index, response) in responses.into_iter().enumerate() {
        let offset = segment_duration.as_secs_f64() * index as f64;
        let segment_speaker_labels = response
            .metadata
            .get("speaker_labels")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        speaker_labels.extend(segment_speaker_labels.iter().cloned());
        speaker_segments.extend(
            response
                .metadata
                .get("speaker_segments")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .cloned()
                .map(|mut segment| {
                    for field in ["start", "end"] {
                        if let Some(value) = segment.get_mut(field)
                            && let Some(time) = value.as_f64()
                        {
                            *value = serde_json::json!(time + offset);
                        }
                    }
                    segment
                }),
        );
        if metadata.is_null() {
            metadata = response.metadata;
        }

        let Some(alternative) = response
            .results
            .channels
            .into_iter()
            .next()
            .and_then(|channel| channel.alternatives.into_iter().next())
        else {
            continue;
        };

        let transcript = alternative.transcript.trim();
        if !transcript.is_empty() {
            transcripts.push(transcript.to_string());
        }
        let segment_speaker_count = alternative
            .words
            .iter()
            .filter_map(|word| word.speaker)
            .max()
            .map_or(0, |speaker| speaker + 1)
            .max(segment_speaker_labels.len());
        words.extend(alternative.words.into_iter().map(|mut word| {
            word.start += offset;
            word.end += offset;
            word.speaker = word.speaker.map(|speaker| speaker + speaker_offset);
            word
        }));
        speaker_offset += segment_speaker_count;
    }

    if let Some(object) = metadata.as_object_mut() {
        if !speaker_labels.is_empty() {
            object.insert(
                "speaker_labels".to_string(),
                serde_json::Value::Array(speaker_labels),
            );
        }
        if !speaker_segments.is_empty() {
            object.insert(
                "speaker_segments".to_string(),
                serde_json::Value::Array(speaker_segments),
            );
        }
    }

    Response {
        metadata: if metadata.is_null() {
            serde_json::json!({})
        } else {
            metadata
        },
        results: Results {
            channels: vec![Channel {
                alternatives: vec![Alternatives {
                    transcript: transcripts.join(" "),
                    confidence: 1.0,
                    words,
                }],
            }],
        },
    }
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
