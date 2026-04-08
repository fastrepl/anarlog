use std::time::Duration;

use axum::{
    Json,
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use backon::{ExponentialBuilder, Retryable};
use futures_util::StreamExt;
use hypr_transcribe_core::batch_sse_response;
use owhisper_client::{
    AssemblyAIAdapter, BatchClient, DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter,
    GladiaAdapter, MistralAdapter, OpenAIAdapter, Provider, PyannoteAdapter, SonioxAdapter,
};
use owhisper_interface::ListenParams;
use owhisper_interface::batch::Response as BatchResponse;
use owhisper_interface::batch_sse::BatchSseMessage;

use crate::hyprnote_routing::{RetryConfig, RoutingMode};
use crate::provider_selector::SelectedProvider;
use crate::query_params::QueryParams;

use super::super::AppState;
use super::super::model_resolution::resolve_model_batch;
use super::write_to_temp_file;

#[derive(Debug, Clone)]
pub(super) enum BatchAttemptError {
    Auth(String),
    Client(String),
    Retryable(String),
    Unsupported(String),
}

impl BatchAttemptError {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }

    pub(super) fn message(&self) -> &str {
        match self {
            Self::Auth(s) | Self::Client(s) | Self::Retryable(s) | Self::Unsupported(s) => s,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Auth(_) => "auth",
            Self::Client(_) => "client",
            Self::Retryable(_) => "retryable",
            Self::Unsupported(_) => "unsupported",
        }
    }
}

impl std::fmt::Display for BatchAttemptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

#[derive(Debug, serde::Serialize)]
struct BatchRoutingTrace {
    request_model: Option<String>,
    request_languages: Vec<String>,
    provider_chain: Vec<String>,
    attempts: Vec<BatchRoutingAttempt>,
    outcome: String,
}

#[derive(Debug, serde::Serialize)]
struct BatchRoutingAttempt {
    provider: String,
    resolved_model: Option<String>,
    retries: usize,
    result: String,
}

fn log_batch_routing_trace(trace: &BatchRoutingTrace, success: bool) {
    let trace_json = serde_json::to_string(trace).unwrap_or_else(|e| {
        serde_json::json!({
            "trace_serialization_error": e.to_string(),
        })
        .to_string()
    });
    if success {
        tracing::info!(trace_json = %trace_json, "hyprnote_batch_routing_trace");
    } else {
        tracing::error!(trace_json = %trace_json, "hyprnote_batch_routing_trace");
    }
}

fn resolve_listen_params_for_provider(
    provider: Provider,
    listen_params: &ListenParams,
) -> ListenParams {
    let mut resolved_params = listen_params.clone();
    resolve_model_batch(provider, &mut resolved_params);
    resolved_params
}

struct PreparedBatchRoute {
    provider_chain: Vec<SelectedProvider>,
    retry_config: RetryConfig,
    trace: BatchRoutingTrace,
}

struct BatchChainFailure {
    last_error: Option<String>,
    providers_tried: Vec<Provider>,
    trace: BatchRoutingTrace,
}

enum AttemptOutcome<T> {
    Success {
        value: T,
        retries: usize,
    },
    Failure {
        error: BatchAttemptError,
        retries: usize,
        can_fallback: bool,
    },
}

fn prepare_routed_batch(
    state: &AppState,
    params: &QueryParams,
    listen_params: &ListenParams,
    body: &Bytes,
    content_type: &str,
) -> Result<PreparedBatchRoute, String> {
    let provider_chain = state.resolve_hyprnote_provider_chain_for_mode(RoutingMode::Batch, params);

    if provider_chain.is_empty() {
        return Err("No providers available for the requested language(s)".to_string());
    }

    let retry_config = state
        .router
        .as_ref()
        .map(|r| r.retry_config().clone())
        .unwrap_or_default();

    tracing::info!(
        provider_chain = ?provider_chain.iter().map(|p| p.provider()).collect::<Vec<_>>(),
        content_type = %content_type,
        body_size_bytes = %body.len(),
        "hyprnote_batch_transcription_request"
    );

    Ok(PreparedBatchRoute {
        trace: BatchRoutingTrace {
            request_model: listen_params.model.clone(),
            request_languages: listen_params
                .languages
                .iter()
                .map(|lang| lang.iso639().code().to_string())
                .collect(),
            provider_chain: provider_chain
                .iter()
                .map(|selected| selected.provider().to_string())
                .collect(),
            attempts: Vec::new(),
            outcome: "in_progress".to_string(),
        },
        provider_chain,
        retry_config,
    })
}

async fn run_routed_batch_chain<T, F, Fut>(
    prepared: PreparedBatchRoute,
    listen_params: &ListenParams,
    body: &Bytes,
    content_type: &str,
    mut attempt_provider: F,
) -> Result<T, BatchChainFailure>
where
    F: FnMut(SelectedProvider, ListenParams, Bytes, String, RetryConfig) -> Fut,
    Fut: std::future::Future<Output = AttemptOutcome<T>>,
{
    let mut last_error: Option<String> = None;
    let mut providers_tried = Vec::new();
    let mut trace = prepared.trace;

    for (attempt, selected) in prepared.provider_chain.iter().enumerate() {
        let provider = selected.provider();
        let provider_listen_params = resolve_listen_params_for_provider(provider, listen_params);
        let resolved_model = provider_listen_params.model.clone();
        providers_tried.push(provider);

        match attempt_provider(
            selected.clone(),
            provider_listen_params,
            body.clone(),
            content_type.to_string(),
            prepared.retry_config.clone(),
        )
        .await
        {
            AttemptOutcome::Success { value, retries } => {
                tracing::info!(
                    hyprnote.stt.provider.name = ?provider,
                    hyprnote.attempt.number = attempt + 1,
                    "batch_transcription_succeeded"
                );
                trace.attempts.push(BatchRoutingAttempt {
                    provider: provider.to_string(),
                    resolved_model,
                    retries,
                    result: "success".to_string(),
                });
                trace.outcome = "success".to_string();
                log_batch_routing_trace(&trace, true);

                return Ok(value);
            }
            AttemptOutcome::Failure {
                error: e,
                retries,
                can_fallback,
            } => {
                tracing::warn!(
                    hyprnote.stt.provider.name = ?provider,
                    error = %e,
                    hyprnote.attempt.number = attempt + 1,
                    hyprnote.remaining_provider_count = prepared.provider_chain.len() - attempt - 1,
                    "provider_failed_trying_next"
                );
                trace.attempts.push(BatchRoutingAttempt {
                    provider: provider.to_string(),
                    resolved_model,
                    retries,
                    result: format!("{}: {}", e.kind(), e.message()),
                });
                last_error = Some(e.message().to_string());

                if !can_fallback {
                    trace.outcome = "terminal_failure".to_string();
                    log_batch_routing_trace(&trace, false);

                    return Err(BatchChainFailure {
                        last_error,
                        providers_tried,
                        trace,
                    });
                }
            }
        }
    }

    trace.outcome = "all_providers_failed".to_string();
    log_batch_routing_trace(&trace, false);

    Err(BatchChainFailure {
        last_error,
        providers_tried,
        trace,
    })
}

pub(super) fn handle_routed_batch_sse(
    state: &AppState,
    params: &QueryParams,
    listen_params: ListenParams,
    body: Bytes,
    content_type: &str,
) -> Response {
    let prepared = prepare_routed_batch(state, params, &listen_params, &body, content_type);
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<BatchSseMessage>();

    match prepared {
        Ok(prepared) => {
            let content_type = content_type.to_string();
            tokio::spawn(async move {
                let result = run_routed_batch_chain(
                    prepared,
                    &listen_params,
                    &body,
                    &content_type,
                    |selected, params, body, content_type, retry_config| {
                        let event_tx = event_tx.clone();
                        async move {
                            stream_batch_attempt_as_sse(
                                &selected,
                                params,
                                &body,
                                &content_type,
                                &retry_config,
                                &event_tx,
                            )
                            .await
                        }
                    },
                )
                .await;

                if let Err(failure) = result {
                    let detail = failure
                        .last_error
                        .unwrap_or_else(|| "Unknown error".to_string());
                    let error = if failure.trace.provider_chain.is_empty() {
                        "no_providers_available".to_string()
                    } else {
                        "all_providers_failed".to_string()
                    };
                    let _ = event_tx.send(BatchSseMessage::Error { error, detail });
                }
            });
        }
        Err(detail) => {
            let _ = event_tx.send(BatchSseMessage::Error {
                error: "no_providers_available".to_string(),
                detail,
            });
        }
    }

    batch_sse_response(event_rx)
}

pub(super) fn handle_direct_batch_sse(
    selected: SelectedProvider,
    listen_params: ListenParams,
    body: Bytes,
    content_type: &str,
    retry_config: RetryConfig,
) -> Response {
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<BatchSseMessage>();
    let content_type = content_type.to_string();

    tokio::spawn(async move {
        let outcome = stream_batch_attempt_as_sse(
            &selected,
            listen_params,
            &body,
            &content_type,
            &retry_config,
            &event_tx,
        )
        .await;

        if let AttemptOutcome::Failure { error, .. } = outcome {
            let _ = event_tx.send(BatchSseMessage::Error {
                error: selected.provider().to_string(),
                detail: error.to_string(),
            });
        }
    });

    batch_sse_response(event_rx)
}

pub(super) async fn handle_routed_batch_json(
    state: &AppState,
    params: &QueryParams,
    listen_params: ListenParams,
    body: Bytes,
    content_type: &str,
) -> Response {
    let prepared = match prepare_routed_batch(state, params, &listen_params, &body, content_type) {
        Ok(prepared) => prepared,
        Err(detail) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "no_providers_available",
                    "detail": detail
                })),
            )
                .into_response();
        }
    };

    match run_routed_batch_chain(
        prepared,
        &listen_params,
        &body,
        content_type,
        |selected, params, body, content_type, retry_config| async move {
            match transcribe_with_retry(&selected, params, body, &content_type, &retry_config).await
            {
                Ok((response, retries)) => AttemptOutcome::Success {
                    value: response,
                    retries,
                },
                Err((error, retries)) => AttemptOutcome::Failure {
                    error,
                    retries,
                    can_fallback: true,
                },
            }
        },
    )
    .await
    {
        Ok(response) => Json(response).into_response(),
        Err(failure) => all_providers_failed_response(failure),
    }
}

async fn stream_batch_attempt_as_sse(
    selected: &SelectedProvider,
    params: ListenParams,
    audio_bytes: &Bytes,
    content_type: &str,
    retry_config: &RetryConfig,
    event_tx: &tokio::sync::mpsc::UnboundedSender<BatchSseMessage>,
) -> AttemptOutcome<()> {
    if selected.provider() == Provider::OpenAI
        && OpenAIAdapter::supports_progressive_batch_model(params.model.as_deref())
    {
        return stream_openai_batch_sse_with_retry(
            selected,
            params,
            audio_bytes,
            content_type,
            retry_config,
            event_tx,
        )
        .await;
    }

    match transcribe_with_retry(
        selected,
        params,
        audio_bytes.clone(),
        content_type,
        retry_config,
    )
    .await
    {
        Ok((response, retries)) => {
            let _ = event_tx.send(BatchSseMessage::Result { response });
            AttemptOutcome::Success { value: (), retries }
        }
        Err((error, retries)) => AttemptOutcome::Failure {
            error,
            retries,
            can_fallback: true,
        },
    }
}

struct StreamingAttemptFailure {
    error: BatchAttemptError,
    emitted_output: bool,
}

async fn stream_openai_batch_sse_with_retry(
    selected: &SelectedProvider,
    params: ListenParams,
    audio_bytes: &Bytes,
    content_type: &str,
    retry_config: &RetryConfig,
    event_tx: &tokio::sync::mpsc::UnboundedSender<BatchSseMessage>,
) -> AttemptOutcome<()> {
    let mut retries = 0usize;

    loop {
        match stream_openai_batch_sse_once(selected, &params, audio_bytes, content_type, event_tx)
            .await
        {
            Ok(()) => {
                return AttemptOutcome::Success { value: (), retries };
            }
            Err(failure)
                if !failure.emitted_output
                    && failure.error.is_retryable()
                    && retries < retry_config.num_retries =>
            {
                retries += 1;
                let delay_secs = (1u64 << retries).min(retry_config.max_delay_secs);
                let delay = Duration::from_secs(delay_secs);
                tracing::warn!(
                    hyprnote.stt.provider.name = ?selected.provider(),
                    error = %failure.error,
                    hyprnote.retry.delay_ms = delay.as_millis(),
                    "retrying_transcription"
                );
                tokio::time::sleep(delay).await;
            }
            Err(failure) => {
                if failure.emitted_output {
                    let _ = event_tx.send(BatchSseMessage::Error {
                        error: selected.provider().to_string(),
                        detail: failure.error.to_string(),
                    });
                }

                return AttemptOutcome::Failure {
                    error: failure.error,
                    retries,
                    can_fallback: !failure.emitted_output,
                };
            }
        }
    }
}

async fn stream_openai_batch_sse_once(
    selected: &SelectedProvider,
    params: &ListenParams,
    audio_bytes: &Bytes,
    content_type: &str,
    event_tx: &tokio::sync::mpsc::UnboundedSender<BatchSseMessage>,
) -> Result<(), StreamingAttemptFailure> {
    let temp_file =
        write_to_temp_file(audio_bytes, content_type).map_err(|e| StreamingAttemptFailure {
            error: BatchAttemptError::Client(format!("failed to create temp file: {e}")),
            emitted_output: false,
        })?;

    let file_path = temp_file.path().to_path_buf();
    let provider_name = selected.provider().to_string();
    let api_base = selected
        .upstream_url()
        .unwrap_or(selected.provider().default_api_base());
    let api_key = selected.api_key();

    let mut stream =
        OpenAIAdapter::transcribe_file_streaming(api_base, api_key, params, &file_path)
            .await
            .map_err(|error| StreamingAttemptFailure {
                error: map_provider_error(error),
                emitted_output: false,
            })?;

    let mut emitted_output = false;

    while let Some(event) = stream.next().await {
        match event {
            Ok(owhisper_interface::batch_stream::BatchStreamEvent::Progress {
                percentage,
                partial_text,
            }) => {
                emitted_output = true;
                let _ = event_tx.send(BatchSseMessage::Progress {
                    progress: owhisper_interface::InferenceProgress {
                        percentage,
                        partial_text,
                        phase: owhisper_interface::progress::InferencePhase::Transcribing,
                    },
                });
            }
            Ok(owhisper_interface::batch_stream::BatchStreamEvent::Result { response }) => {
                emitted_output = true;
                let _ = event_tx.send(BatchSseMessage::Result { response });
            }
            Ok(owhisper_interface::batch_stream::BatchStreamEvent::Error {
                error_message,
                provider,
                ..
            }) => {
                return Err(StreamingAttemptFailure {
                    error: classify_audio_processing_message(if provider.is_empty() {
                        error_message
                    } else {
                        format!("{provider}: {error_message}")
                    }),
                    emitted_output,
                });
            }
            Ok(owhisper_interface::batch_stream::BatchStreamEvent::Segment { .. })
            | Ok(owhisper_interface::batch_stream::BatchStreamEvent::Terminal { .. }) => {}
            Err(error) => {
                return Err(StreamingAttemptFailure {
                    error: map_provider_error(error),
                    emitted_output,
                });
            }
        }
    }

    if !emitted_output {
        return Err(StreamingAttemptFailure {
            error: BatchAttemptError::Retryable(format!(
                "{provider_name} stream ended before emitting any events"
            )),
            emitted_output: false,
        });
    }

    Ok(())
}

fn all_providers_failed_response(failure: BatchChainFailure) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({
            "error": "all_providers_failed",
            "detail": failure.last_error.unwrap_or_else(|| "Unknown error".to_string()),
            "providers_tried": failure.providers_tried.iter().map(|p| format!("{:?}", p)).collect::<Vec<_>>()
        })),
    )
        .into_response()
}

pub(super) async fn transcribe_with_retry(
    selected: &SelectedProvider,
    params: ListenParams,
    audio_bytes: Bytes,
    content_type: &str,
    retry_config: &RetryConfig,
) -> Result<(BatchResponse, usize), (BatchAttemptError, usize)> {
    let backoff = ExponentialBuilder::default()
        .with_jitter()
        .with_max_delay(Duration::from_secs(retry_config.max_delay_secs))
        .with_max_times(retry_config.num_retries);
    let mut retries = 0usize;

    let result = (|| async {
        transcribe_with_provider(selected, params.clone(), audio_bytes.clone(), content_type).await
    })
    .retry(backoff)
    .notify(|err, dur| {
        tracing::warn!(
            hyprnote.stt.provider.name = ?selected.provider(),
            error = %err,
            hyprnote.retry.delay_ms = dur.as_millis(),
            "retrying_transcription"
        );
        retries += 1;
    })
    .when(|e| e.is_retryable())
    .await;

    match result {
        Ok(response) => Ok((response, retries)),
        Err(err) => Err((err, retries)),
    }
}

pub(super) async fn transcribe_with_provider(
    selected: &SelectedProvider,
    params: ListenParams,
    audio_bytes: Bytes,
    content_type: &str,
) -> Result<BatchResponse, BatchAttemptError> {
    let temp_file = write_to_temp_file(&audio_bytes, content_type)
        .map_err(|e| BatchAttemptError::Client(format!("failed to create temp file: {e}")))?;

    let file_path = temp_file.path();
    let provider = selected.provider();
    let api_base = selected
        .upstream_url()
        .unwrap_or(provider.default_api_base());
    let api_key = selected.api_key();

    macro_rules! batch_transcribe {
        ($adapter:ty) => {
            BatchClient::<$adapter>::builder()
                .api_base(api_base)
                .api_key(api_key)
                .params(params)
                .build()
                .transcribe_file(file_path)
                .await
        };
    }

    let result = match provider {
        Provider::Deepgram => batch_transcribe!(DeepgramAdapter),
        Provider::AssemblyAI => batch_transcribe!(AssemblyAIAdapter),
        Provider::Soniox => batch_transcribe!(SonioxAdapter),
        Provider::OpenAI => batch_transcribe!(OpenAIAdapter),
        Provider::Gladia => batch_transcribe!(GladiaAdapter),
        Provider::ElevenLabs => batch_transcribe!(ElevenLabsAdapter),
        Provider::Mistral => batch_transcribe!(MistralAdapter),
        Provider::Pyannote => batch_transcribe!(PyannoteAdapter),
        Provider::Fireworks => batch_transcribe!(FireworksAdapter),
        Provider::DashScope => {
            return Err(BatchAttemptError::Unsupported(format!(
                "{provider:?} does not support batch transcription",
            )));
        }
    };

    result.map_err(map_provider_error)
}

fn map_provider_error(error: owhisper_client::Error) -> BatchAttemptError {
    match error {
        owhisper_client::Error::UnexpectedStatus { status, body } => classify_http_status(
            status.as_u16(),
            format!("unexpected response status {status}: {body}"),
        ),
        owhisper_client::Error::Http(err) => map_http_error(err),
        owhisper_client::Error::HttpMiddleware(err) => {
            BatchAttemptError::Retryable(format!("http middleware error: {err}"))
        }
        owhisper_client::Error::Task(err) => {
            BatchAttemptError::Retryable(format!("task join error: {err}"))
        }
        owhisper_client::Error::ProviderFailure {
            message,
            retryable,
            status,
        } => {
            if let Some(status) = status {
                classify_http_status(status.as_u16(), message)
            } else if retryable {
                BatchAttemptError::Retryable(message)
            } else {
                BatchAttemptError::Client(message)
            }
        }
        owhisper_client::Error::AudioProcessing(msg) => classify_audio_processing_message(msg),
        owhisper_client::Error::WebSocket(msg) => BatchAttemptError::Retryable(msg),
    }
}

fn map_http_error(err: reqwest::Error) -> BatchAttemptError {
    if err.is_timeout() || err.is_connect() {
        return BatchAttemptError::Retryable(err.to_string());
    }

    if let Some(status) = err.status() {
        return classify_http_status(status.as_u16(), err.to_string());
    }

    BatchAttemptError::Retryable(err.to_string())
}

fn classify_http_status(status: u16, message: String) -> BatchAttemptError {
    match status {
        401 | 403 => BatchAttemptError::Auth(message),
        429 => BatchAttemptError::Retryable(message),
        400..=499 => BatchAttemptError::Client(message),
        500..=599 => BatchAttemptError::Retryable(message),
        _ => BatchAttemptError::Retryable(message),
    }
}

fn classify_audio_processing_message(message: String) -> BatchAttemptError {
    let error_lower = message.to_lowercase();

    let is_auth_error = error_lower.contains("401")
        || error_lower.contains("403")
        || error_lower.contains("unauthorized")
        || error_lower.contains("forbidden");

    let is_client_error = error_lower.contains("400") || error_lower.contains("invalid");

    if is_auth_error {
        return BatchAttemptError::Auth(message);
    }

    if is_client_error {
        return BatchAttemptError::Client(message);
    }

    let is_retryable = error_lower.contains("timeout")
        || error_lower.contains("timed out")
        || error_lower.contains("connection")
        || error_lower.contains("network")
        || error_lower.contains("500")
        || error_lower.contains("502")
        || error_lower.contains("503")
        || error_lower.contains("504")
        || error_lower.contains("temporarily")
        || error_lower.contains("rate limit")
        || error_lower.contains("too many requests");

    if is_retryable {
        BatchAttemptError::Retryable(message)
    } else {
        BatchAttemptError::Client(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_language::ISO639;

    #[test]
    fn test_resolve_listen_params_for_provider_resolves_meta_model_per_provider() {
        let params = ListenParams {
            model: Some("cloud".to_string()),
            languages: vec![ISO639::En.into()],
            ..Default::default()
        };

        let deepgram_params = resolve_listen_params_for_provider(Provider::Deepgram, &params);
        assert!(deepgram_params.model.is_some());
        assert_ne!(deepgram_params.model.as_deref(), Some("cloud"));

        let soniox_params = resolve_listen_params_for_provider(Provider::Soniox, &params);
        assert_eq!(soniox_params.model, None);

        assert_eq!(params.model.as_deref(), Some("cloud"));
    }

    #[test]
    fn test_classify_audio_processing_timeout_is_retryable() {
        let classified = classify_audio_processing_message("request timed out".to_string());
        assert!(matches!(classified, BatchAttemptError::Retryable(_)));
    }

    #[test]
    fn test_classify_audio_processing_invalid_is_client() {
        let classified = classify_audio_processing_message("invalid language".to_string());
        assert!(matches!(classified, BatchAttemptError::Client(_)));
    }

    #[test]
    fn test_provider_failure_retryable_maps_to_retryable() {
        let err = map_provider_error(owhisper_client::Error::ProviderFailure {
            message: "transient upstream failure".to_string(),
            retryable: true,
            status: None,
        });
        assert!(matches!(err, BatchAttemptError::Retryable(_)));
    }

    #[test]
    fn test_provider_failure_with_status_401_maps_to_auth() {
        let err = map_provider_error(owhisper_client::Error::ProviderFailure {
            message: "unauthorized".to_string(),
            retryable: true,
            status: Some(reqwest::StatusCode::UNAUTHORIZED),
        });
        assert!(matches!(err, BatchAttemptError::Auth(_)));
    }
}
