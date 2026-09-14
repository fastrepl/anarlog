use std::path::Path;
use std::time::Duration;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use backon::{ExponentialBuilder, Retryable};
use owhisper_client::{
    AquaVoiceAdapter, AssemblyAIAdapter, BatchClient, CartesiaAdapter, CohereAdapter,
    DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, MistralAdapter,
    OpenAIAdapter, Provider, PyannoteAdapter, SonioxAdapter,
};
use owhisper_interface::ListenParams;
use owhisper_interface::batch::Response as BatchResponse;

use crate::anarlog_routing::{RetryConfig, RoutingMode};
use crate::provider_selector::SelectedProvider;
use crate::query_params::QueryParams;

use super::super::AppState;
use super::super::model_resolution::resolve_model_batch;

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

    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::Auth(_) => "auth",
            Self::Client(_) => "client",
            Self::Retryable(_) => "retryable",
            Self::Unsupported(_) => "unsupported",
        }
    }

    pub(super) fn is_user_error(&self) -> bool {
        matches!(self, Self::Auth(_) | Self::Client(_) | Self::Unsupported(_))
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
    mixes_channels: bool,
    retries: usize,
    result: String,
}

fn log_batch_routing_trace(trace: &BatchRoutingTrace, success: bool, user_error: bool) {
    let trace_json = serde_json::to_string(trace).unwrap_or_else(|e| {
        serde_json::json!({
            "trace_serialization_error": e.to_string(),
        })
        .to_string()
    });
    if success {
        tracing::info!(trace_json = %trace_json, "anarlog_batch_routing_trace");
    } else if user_error {
        tracing::warn!(trace_json = %trace_json, "anarlog_batch_routing_trace");
    } else {
        tracing::error!(trace_json = %trace_json, "anarlog_batch_routing_trace");
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

/// A stereo capture keeps the direct mic on channel 0 and the remote party on
/// channel 1. Providers that downmix return one mixed channel instead, which the
/// desktop can still diarize, so for a single-language request they run only
/// after every provider that keeps the split has been tried. A request that
/// spans several languages keeps the router's order instead: the
/// channel-preserving providers transcribe batch audio in one detected language,
/// so promoting them would drop the other language's speech, which is worse than
/// losing the channel split. The sort is stable within each group.
fn prefer_channel_preserving_providers(
    provider_chain: &mut [SelectedProvider],
    listen_params: &ListenParams,
) {
    if listen_params.channels > 1 && listen_params.languages.len() <= 1 {
        provider_chain
            .sort_by_key(|selected| !selected.provider().preserves_batch_channel_identity());
    }
}

pub(super) async fn handle_anarlog_batch(
    state: &AppState,
    params: &QueryParams,
    listen_params: ListenParams,
    audio_path: &Path,
    audio_size_bytes: u64,
    content_type: &str,
    max_response_bytes: Option<usize>,
) -> Response {
    let mut provider_chain =
        state.resolve_anarlog_provider_chain_for_mode(RoutingMode::Batch, params);
    prefer_channel_preserving_providers(&mut provider_chain, &listen_params);

    if provider_chain.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "no_providers_available",
                "detail": "No providers available for the requested language(s)"
            })),
        )
            .into_response();
    }

    let retry_config = state
        .router
        .as_ref()
        .map(|r| r.retry_config().clone())
        .unwrap_or_default();

    tracing::info!(
        provider_chain = ?provider_chain.iter().map(|p| p.provider()).collect::<Vec<_>>(),
        content_type = %content_type,
        body_size_bytes = %audio_size_bytes,
        "anarlog_batch_transcription_request"
    );

    let mut last_error: Option<String> = None;
    let mut last_error_is_user = false;
    let mut providers_tried = Vec::new();
    let mut trace = BatchRoutingTrace {
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
    };

    for (attempt, selected) in provider_chain.iter().enumerate() {
        let provider = selected.provider();
        let provider_listen_params = resolve_listen_params_for_provider(provider, &listen_params);
        let resolved_model = provider_listen_params.model.clone();
        let mixes_channels =
            provider_listen_params.channels > 1 && !provider.preserves_batch_channel_identity();
        providers_tried.push(provider);
        if mixes_channels {
            tracing::info!(
                anarlog.stt.provider.name = ?provider,
                anarlog.attempt.number = attempt + 1,
                "multichannel_batch_downmixed_by_provider"
            );
        }

        match transcribe_with_retry(selected, provider_listen_params, audio_path, &retry_config)
            .await
        {
            Ok((response, retries)) => {
                tracing::info!(
                    anarlog.stt.provider.name = ?provider,
                    anarlog.attempt.number = attempt + 1,
                    "batch_transcription_succeeded"
                );
                trace.attempts.push(BatchRoutingAttempt {
                    provider: provider.to_string(),
                    resolved_model,
                    mixes_channels,
                    retries,
                    result: "success".to_string(),
                });
                trace.outcome = "success".to_string();
                log_batch_routing_trace(&trace, true, false);

                return super::bounded_json_response(response, max_response_bytes);
            }
            Err((e, retries)) => {
                tracing::warn!(
                    anarlog.stt.provider.name = ?provider,
                    error.type = e.kind(),
                    anarlog.attempt.number = attempt + 1,
                    anarlog.remaining_provider_count = provider_chain.len() - attempt - 1,
                    "provider_failed_trying_next"
                );
                trace.attempts.push(BatchRoutingAttempt {
                    provider: provider.to_string(),
                    resolved_model,
                    mixes_channels,
                    retries,
                    result: e.kind().to_string(),
                });
                last_error = Some(e.message().to_string());
                last_error_is_user = e.is_user_error();
            }
        }
    }

    trace.outcome = "all_providers_failed".to_string();
    log_batch_routing_trace(&trace, false, last_error_is_user);

    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({
            "error": "all_providers_failed",
            "detail": last_error.unwrap_or_else(|| "Unknown error".to_string()),
            "providers_tried": providers_tried.iter().map(|p| format!("{:?}", p)).collect::<Vec<_>>()
        })),
    )
        .into_response()
}

pub(super) async fn transcribe_with_retry(
    selected: &SelectedProvider,
    params: ListenParams,
    audio_path: &Path,
    retry_config: &RetryConfig,
) -> Result<(BatchResponse, usize), (BatchAttemptError, usize)> {
    let backoff = ExponentialBuilder::default()
        .with_jitter()
        .with_max_delay(Duration::from_secs(retry_config.max_delay_secs))
        .with_max_times(retry_config.num_retries);
    let mut retries = 0usize;

    let result =
        (|| async { transcribe_with_provider(selected, params.clone(), audio_path).await })
            .retry(backoff)
            .notify(|err, dur| {
                tracing::warn!(
                    anarlog.stt.provider.name = ?selected.provider(),
                    error.type = err.kind(),
                    anarlog.retry.delay_ms = dur.as_millis(),
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
    audio_path: &Path,
) -> Result<BatchResponse, BatchAttemptError> {
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
                .transcribe_file(audio_path)
                .await
        };
    }

    let result = match provider {
        Provider::Deepgram => batch_transcribe!(DeepgramAdapter),
        Provider::AssemblyAI => batch_transcribe!(AssemblyAIAdapter),
        Provider::Cartesia => batch_transcribe!(CartesiaAdapter),
        Provider::Soniox => batch_transcribe!(SonioxAdapter),
        Provider::OpenAI => batch_transcribe!(OpenAIAdapter),
        Provider::Gladia => batch_transcribe!(GladiaAdapter),
        Provider::ElevenLabs => batch_transcribe!(ElevenLabsAdapter),
        Provider::Mistral => batch_transcribe!(MistralAdapter),
        Provider::Pyannote => batch_transcribe!(PyannoteAdapter),
        Provider::Fireworks => batch_transcribe!(FireworksAdapter),
        Provider::AquaVoice => batch_transcribe!(AquaVoiceAdapter),
        Provider::Cohere => batch_transcribe!(CohereAdapter),
        Provider::DashScope => {
            return Err(BatchAttemptError::Unsupported(format!(
                "{provider:?} does not support batch transcription",
            )));
        }
        Provider::AwsTranscribe
        | Provider::AzureSpeech
        | Provider::GoogleCloud
        | Provider::Groq
        | Provider::RevAi
        | Provider::Speechmatics
        | Provider::Together
        | Provider::Xai
        | Provider::Nari
        | Provider::SmallestAI
        | Provider::Meta
        | Provider::GoogleGenerativeAi => {
            return Err(BatchAttemptError::Unsupported(format!(
                "{provider:?} is a direct BYOK provider",
            )));
        }
    };

    result.map_err(map_provider_error)
}

fn map_provider_error(error: owhisper_client::Error) -> BatchAttemptError {
    match error {
        owhisper_client::Error::UnexpectedStatus { status, body } => {
            classify_http_status(status.as_u16(), &body)
        }
        owhisper_client::Error::Http(err) => map_http_error(err),
        owhisper_client::Error::HttpMiddleware(_) => {
            BatchAttemptError::Retryable("provider transport unavailable".to_string())
        }
        owhisper_client::Error::Task(_) => {
            BatchAttemptError::Retryable("provider task unavailable".to_string())
        }
        owhisper_client::Error::ProviderFailure {
            message,
            retryable,
            status,
        } => {
            if let Some(status) = status {
                classify_http_status(status.as_u16(), &message)
            } else if anlg_user_error::is_user_error_text(&message) {
                BatchAttemptError::Client("provider account action required".to_string())
            } else if retryable {
                BatchAttemptError::Retryable("provider unavailable".to_string())
            } else {
                BatchAttemptError::Client("provider request rejected".to_string())
            }
        }
        owhisper_client::Error::ProviderConfiguration { provider, .. } => {
            BatchAttemptError::Client(format!("invalid endpoint configuration for {provider}"))
        }
        owhisper_client::Error::AudioProcessing(msg) => classify_audio_processing_message(msg),
        owhisper_client::Error::WebSocket(_) => {
            BatchAttemptError::Retryable("provider websocket unavailable".to_string())
        }
    }
}

fn map_http_error(err: reqwest::Error) -> BatchAttemptError {
    if err.is_timeout() || err.is_connect() {
        return BatchAttemptError::Retryable("provider transport unavailable".to_string());
    }

    if let Some(status) = err.status() {
        return classify_http_status(status.as_u16(), "");
    }

    BatchAttemptError::Retryable("provider transport unavailable".to_string())
}

fn classify_http_status(status: u16, body: &str) -> BatchAttemptError {
    match status {
        401 | 403 => BatchAttemptError::Auth("invalid api key".to_string()),
        429 if anlg_user_error::is_user_error_text(body) => {
            BatchAttemptError::Client("quota exceeded".to_string())
        }
        429 => BatchAttemptError::Retryable("provider rate limited".to_string()),
        400..=499 => BatchAttemptError::Client("provider request rejected".to_string()),
        500..=599 => BatchAttemptError::Retryable("provider unavailable".to_string()),
        _ => BatchAttemptError::Retryable("unexpected provider response".to_string()),
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
        return BatchAttemptError::Auth("invalid api key".to_string());
    }

    if is_client_error {
        return BatchAttemptError::Client("invalid audio request".to_string());
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
        BatchAttemptError::Retryable("provider unavailable".to_string())
    } else {
        BatchAttemptError::Client("audio processing failed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_language::ISO639;

    fn test_state(providers: &[Provider]) -> AppState {
        use crate::config::{CallbackConfig, SttProxyConfig, SupabaseConfig};

        super::super::super::make_state(
            SttProxyConfig {
                api_keys: providers
                    .iter()
                    .map(|provider| (*provider, "test-key".to_string()))
                    .collect(),
                default_provider: Provider::Deepgram,
                connect_timeout: Duration::from_secs(1),
                analytics: None,
                upstream_urls: Default::default(),
                anarlog_routing: Some(Default::default()),
                supabase: SupabaseConfig {
                    url: None,
                    service_role_key: None,
                },
                callback: CallbackConfig {
                    api_base_url: None,
                    secret: None,
                },
            },
            Default::default(),
        )
    }

    fn language_params(codes: &[&str]) -> QueryParams {
        use crate::query_params::QueryValue;

        let mut params = QueryParams::default();
        params.insert(
            "language".to_string(),
            QueryValue::Multi(codes.iter().map(|code| (*code).to_string()).collect()),
        );
        params
    }

    fn chain_providers(chain: &[SelectedProvider]) -> Vec<Provider> {
        chain.iter().map(SelectedProvider::provider).collect()
    }

    fn ordered_chain(state: &AppState, codes: &[&str], channels: u8) -> Vec<Provider> {
        let params = language_params(codes);
        let mut chain = state.resolve_anarlog_provider_chain_for_mode(RoutingMode::Batch, &params);
        prefer_channel_preserving_providers(
            &mut chain,
            &ListenParams {
                channels,
                languages: codes
                    .iter()
                    .map(|code| code.parse::<ISO639>().unwrap().into())
                    .collect(),
                ..Default::default()
            },
        );
        chain_providers(&chain)
    }

    #[test]
    fn stereo_batch_orders_channel_preserving_providers_first() {
        let state = test_state(&[Provider::Deepgram, Provider::Soniox]);

        assert_eq!(
            ordered_chain(&state, &["en"], 1),
            vec![Provider::Soniox, Provider::Deepgram]
        );
        assert_eq!(
            ordered_chain(&state, &["en"], 2),
            vec![Provider::Deepgram, Provider::Soniox]
        );
    }

    #[test]
    fn stereo_mixed_language_batch_keeps_full_coverage_provider_first() {
        let state = test_state(&[Provider::Deepgram, Provider::Soniox]);

        // Deepgram only reaches hu+en through language detection, which transcribes
        // one language. Soniox covers both, so it stays ahead even though it mixes
        // the channels.
        assert_eq!(
            ordered_chain(&state, &["hu", "en"], 2),
            vec![Provider::Soniox, Provider::Deepgram]
        );
    }

    #[tokio::test]
    async fn stereo_mixed_language_batch_downmixes_instead_of_failing() {
        let state = test_state(&[Provider::Soniox]);
        let params = language_params(&["hu", "en"]);

        // Soniox must be attempted (the missing file fails its upload) instead of
        // being rejected for downmixing the stereo capture.
        let response = handle_anarlog_batch(
            &state,
            &params,
            ListenParams {
                channels: 2,
                languages: vec![ISO639::Hu.into(), ISO639::En.into()],
                ..Default::default()
            },
            Path::new("missing-stereo-mixed-language-recording.wav"),
            0,
            "audio/wav",
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"], "all_providers_failed");
        assert_eq!(body["providers_tried"], serde_json::json!(["Soniox"]));
        assert_eq!(body["detail"], "audio processing failed");
    }

    #[tokio::test]
    async fn mixed_language_without_full_coverage_provider_falls_back_to_detection() {
        let state = test_state(&[Provider::Deepgram]);
        let params = language_params(&["hu", "en"]);

        // With no provider covering both languages, Deepgram's detection fallback
        // is attempted (the missing file fails it) rather than rejected outright.
        let response = handle_anarlog_batch(
            &state,
            &params,
            ListenParams {
                languages: vec![ISO639::Hu.into(), ISO639::En.into()],
                ..Default::default()
            },
            Path::new("missing-mixed-language-recording.wav"),
            0,
            "audio/wav",
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"], "all_providers_failed");
        assert_eq!(body["providers_tried"], serde_json::json!(["Deepgram"]));
    }

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

    #[test]
    fn provider_response_body_is_not_retained() {
        let secret = "patient@example.com transcript text";
        let err = map_provider_error(owhisper_client::Error::UnexpectedStatus {
            status: reqwest::StatusCode::BAD_REQUEST,
            body: secret.to_string(),
        });

        assert!(!err.message().contains(secret));
        assert_eq!(err.message(), "provider request rejected");
    }

    #[test]
    fn quota_failures_remain_user_errors_without_retaining_the_body() {
        let err = map_provider_error(owhisper_client::Error::UnexpectedStatus {
            status: reqwest::StatusCode::TOO_MANY_REQUESTS,
            body: "insufficient_quota for patient@example.com".to_string(),
        });

        assert!(matches!(err, BatchAttemptError::Client(_)));
        assert_eq!(err.message(), "quota exceeded");
        assert!(anlg_user_error::is_user_error_text(err.message()));
    }

    #[test]
    fn test_provider_configuration_maps_to_non_retryable_client_error() {
        let err = map_provider_error(owhisper_client::Error::ProviderConfiguration {
            provider: "test".to_string(),
            message: "invalid endpoint".to_string(),
        });
        assert!(matches!(err, BatchAttemptError::Client(_)));
    }
}
