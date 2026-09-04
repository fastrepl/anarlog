pub mod async_callback;
mod sync;

use std::io;
use std::path::Path;

use anlg_api_auth::AuthContext;
use axum::{
    Json,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::BytesMut;
use futures_util::StreamExt;
use owhisper_client::normalize_listen_params;
use owhisper_interface::ListenParams;
use tokio::io::AsyncWriteExt;

use anlg_audio_mime::content_type_to_extension;

use crate::anarlog_routing::should_use_anarlog_routing;
use crate::query_params::QueryParams;

use super::{AppState, MAX_BATCH_AUDIO_BODY_BYTES, MAX_BATCH_CALLBACK_BODY_BYTES};

pub async fn handler(
    State(state): State<AppState>,
    auth: Option<axum::Extension<AuthContext>>,
    headers: HeaderMap,
    mut params: QueryParams,
    body: Body,
) -> Response {
    if params.get_first("callback").is_some() {
        let body = match read_callback_body(body).await {
            Ok(body) => body,
            Err(error) => return callback_body_read_error_response(error),
        };

        if body.is_empty() {
            return missing_audio_response();
        }

        return async_callback::handle_callback(&state, auth, &mut params, body)
            .await
            .into_response();
    }

    let _batch_slot = match state.try_acquire_batch_slot() {
        Ok(slot) => slot,
        Err(error) => return error.into_response(),
    };

    let max_response_bytes = params
        .remove_first("max_response_bytes")
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0);

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");

    let listen_params = build_listen_params(&params);
    let audio = match write_body_to_temp_file(body, content_type).await {
        Ok(audio) => audio,
        Err(error) => return audio_write_error_response(error),
    };

    if audio.is_empty() {
        return missing_audio_response();
    }

    let provider_param = params.get_first("provider").map(|s| s.to_string());
    let use_anarlog_routing = should_use_anarlog_routing(provider_param.as_deref());

    if use_anarlog_routing {
        return sync::handle_anarlog_batch(
            &state,
            &params,
            listen_params,
            audio.path(),
            audio.len(),
            content_type,
            max_response_bytes,
        )
        .await;
    }

    let selected = match state.resolve_provider(&mut params) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    tracing::info!(
        anarlog.stt.provider.name = ?selected.provider(),
        anarlog.file.mime_type = %content_type,
        anarlog.payload.size_bytes = %audio.len(),
        "batch_transcription_request_received"
    );

    let retry_config = state
        .router
        .as_ref()
        .map(|r| r.retry_config().clone())
        .unwrap_or_default();

    match sync::transcribe_with_retry(&selected, listen_params, audio.path(), &retry_config).await {
        Ok((response, _retries)) => bounded_json_response(response, max_response_bytes),
        Err((e, _retries)) => {
            if !e.is_user_error() {
                tracing::error!(
                    error.type = e.kind(),
                    anarlog.stt.provider.name = ?selected.provider(),
                    "batch_transcription_failed"
                );
            }
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": "transcription_failed",
                    "detail": e.message()
                })),
            )
                .into_response()
        }
    }
}

pub(super) fn bounded_json_response<T: serde::Serialize>(
    value: T,
    max_response_bytes: Option<usize>,
) -> Response {
    let Some(max_response_bytes) = max_response_bytes else {
        return Json(value).into_response();
    };
    let body = match serde_json::to_vec(&value) {
        Ok(body) => body,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "response_serialization_failed",
                    "detail": error.to_string()
                })),
            )
                .into_response();
        }
    };
    if body.len() > max_response_bytes {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }

    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
}

fn missing_audio_response() -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({
            "error": "missing_audio_data",
            "detail": "Request body is empty"
        })),
    )
        .into_response()
}

fn body_read_error_response(error: axum::Error) -> Response {
    let _ = error;
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({
            "error": "invalid_request_body",
            "detail": "Unable to read request body"
        })),
    )
        .into_response()
}

#[derive(Debug)]
enum CallbackBodyReadError {
    Body(axum::Error),
    TooLarge,
}

fn callback_body_read_error_response(error: CallbackBodyReadError) -> Response {
    match error {
        CallbackBodyReadError::TooLarge => (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({
                "error": "payload_too_large",
                "detail": format!(
                    "Callback request body exceeds {} bytes",
                    MAX_BATCH_CALLBACK_BODY_BYTES
                )
            })),
        )
            .into_response(),
        CallbackBodyReadError::Body(error) => body_read_error_response(error),
    }
}

async fn read_callback_body(body: Body) -> Result<Bytes, CallbackBodyReadError> {
    let mut stream = body.into_data_stream();
    let mut bytes = BytesMut::with_capacity(MAX_BATCH_CALLBACK_BODY_BYTES.min(1024));

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(CallbackBodyReadError::Body)?;
        if bytes.len().saturating_add(chunk.len()) > MAX_BATCH_CALLBACK_BODY_BYTES {
            return Err(CallbackBodyReadError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok(bytes.freeze())
}

pub(super) fn build_listen_params(params: &QueryParams) -> ListenParams {
    normalize_listen_params(ListenParams {
        model: params.get_first("model").map(|s| s.to_string()),
        channels: params
            .parse_optional_u32("channels")
            .and_then(|channels| u8::try_from(channels).ok())
            .unwrap_or(1),
        sample_rate: params.parse_optional_u32("sample_rate").unwrap_or(16_000),
        languages: params.get_languages(),
        keywords: params.parse_keywords(),
        num_speakers: params.parse_optional_u32("num_speakers"),
        min_speakers: params.parse_optional_u32("min_speakers"),
        max_speakers: params.parse_optional_u32("max_speakers"),
        ..Default::default()
    })
}

pub(super) struct BatchAudioFile {
    temp_file: tempfile::NamedTempFile,
    len: u64,
}

impl BatchAudioFile {
    pub(super) fn path(&self) -> &Path {
        self.temp_file.path()
    }

    pub(super) fn len(&self) -> u64 {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Debug)]
enum BatchAudioWriteError {
    Body(axum::Error),
    Io(io::Error),
    TooLarge,
}

impl From<io::Error> for BatchAudioWriteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn audio_write_error_response(error: BatchAudioWriteError) -> Response {
    match error {
        BatchAudioWriteError::TooLarge => (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({
                "error": "payload_too_large",
                "detail": format!("Request body exceeds {} bytes", MAX_BATCH_AUDIO_BODY_BYTES)
            })),
        )
            .into_response(),
        BatchAudioWriteError::Body(error) => body_read_error_response(error),
        BatchAudioWriteError::Io(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "failed_to_store_audio",
                "detail": error.to_string()
            })),
        )
            .into_response(),
    }
}

async fn write_body_to_temp_file(
    body: Body,
    content_type: &str,
) -> Result<BatchAudioFile, BatchAudioWriteError> {
    write_body_to_temp_file_with_limit(body, content_type, MAX_BATCH_AUDIO_BODY_BYTES).await
}

async fn write_body_to_temp_file_with_limit(
    body: Body,
    content_type: &str,
    max_bytes: usize,
) -> Result<BatchAudioFile, BatchAudioWriteError> {
    let extension = content_type_to_extension(content_type);
    let temp_file = tempfile::Builder::new()
        .prefix("batch_audio_")
        .suffix(&format!(".{}", extension))
        .tempfile()?;
    let mut file = tokio::fs::File::from_std(temp_file.reopen()?);
    let mut stream = body.into_data_stream();
    let mut len = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(BatchAudioWriteError::Body)?;
        len += chunk.len() as u64;
        if len > max_bytes as u64 {
            return Err(BatchAudioWriteError::TooLarge);
        }

        file.write_all(&chunk).await?;
    }

    file.flush().await?;

    Ok(BatchAudioFile { temp_file, len })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query_params::QueryValue;
    use anlg_language::ISO639;

    #[test]
    fn test_build_listen_params_normalizes_duplicate_base_languages() {
        let mut params = QueryParams::default();
        params.insert(
            "language".to_string(),
            QueryValue::Multi(vec![
                "en-US".to_string(),
                "en-GB".to_string(),
                "en".to_string(),
                "ko-KR".to_string(),
            ]),
        );

        let listen_params = build_listen_params(&params);

        assert_eq!(listen_params.languages.len(), 2);
        assert_eq!(listen_params.languages[0].iso639(), ISO639::En);
        assert_eq!(listen_params.languages[0].region(), None);
        assert_eq!(listen_params.languages[1].iso639(), ISO639::Ko);
        assert_eq!(listen_params.languages[1].region(), Some("KR"));
    }

    #[test]
    fn test_build_listen_params_with_speaker_counts() {
        let mut params = QueryParams::default();
        params.insert(
            "num_speakers".to_string(),
            QueryValue::Single("3".to_string()),
        );
        params.insert(
            "min_speakers".to_string(),
            QueryValue::Single("2".to_string()),
        );
        params.insert(
            "max_speakers".to_string(),
            QueryValue::Single("4".to_string()),
        );

        let listen_params = build_listen_params(&params);

        assert_eq!(listen_params.num_speakers, Some(3));
        assert_eq!(listen_params.min_speakers, Some(2));
        assert_eq!(listen_params.max_speakers, Some(4));
    }

    #[test]
    fn test_build_listen_params_with_audio_format() {
        let mut params = QueryParams::default();
        params.insert("channels".to_string(), QueryValue::Single("2".to_string()));
        params.insert(
            "sample_rate".to_string(),
            QueryValue::Single("48000".to_string()),
        );

        let listen_params = build_listen_params(&params);

        assert_eq!(listen_params.channels, 2);
        assert_eq!(listen_params.sample_rate, 48_000);
    }

    #[tokio::test]
    async fn test_write_body_to_temp_file_streams_chunks() {
        let body = Body::from_stream(futures_util::stream::iter([
            Ok::<_, io::Error>(Bytes::from_static(b"hello")),
            Ok::<_, io::Error>(Bytes::from_static(b" world")),
        ]));

        let audio = write_body_to_temp_file(body, "audio/wav").await.unwrap();

        assert_eq!(audio.len(), 11);
        assert_eq!(tokio::fs::read(audio.path()).await.unwrap(), b"hello world");
    }

    #[tokio::test]
    async fn test_write_body_to_temp_file_enforces_streaming_limit() {
        let body = Body::from_stream(futures_util::stream::iter([
            Ok::<_, io::Error>(Bytes::from_static(b"hello")),
            Ok::<_, io::Error>(Bytes::from_static(b" world")),
        ]));

        let error = match write_body_to_temp_file_with_limit(body, "audio/wav", 10).await {
            Ok(_) => panic!("oversized stream should be rejected"),
            Err(error) => error,
        };

        assert!(matches!(error, BatchAudioWriteError::TooLarge));
    }

    #[tokio::test]
    async fn test_read_callback_body_accepts_protocol_sized_json() {
        let body = Body::from(r#"{"url":"recordings/meeting.wav"}"#);

        let bytes = read_callback_body(body).await.unwrap();

        assert_eq!(bytes, r#"{"url":"recordings/meeting.wav"}"#);
    }

    #[tokio::test]
    async fn test_read_callback_body_rejects_oversized_payloads() {
        let body = Body::from(vec![0; MAX_BATCH_CALLBACK_BODY_BYTES + 1]);

        let error = read_callback_body(body).await.unwrap_err();

        assert!(matches!(error, CallbackBodyReadError::TooLarge));
    }

    #[tokio::test]
    async fn test_bounded_json_response_rejects_oversized_payloads() {
        let response =
            bounded_json_response(serde_json::json!({ "text": "x".repeat(128) }), Some(32));

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert!(body.len() <= 1024);
    }
}
