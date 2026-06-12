pub mod async_callback;
mod sync;

use std::io::{self, Write};
use std::path::Path;

use axum::{
    Json,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use hypr_api_auth::AuthContext;
use owhisper_client::normalize_listen_params;
use owhisper_interface::ListenParams;
use tokio::io::AsyncWriteExt;

use hypr_audio_mime::content_type_to_extension;

use crate::hyprnote_routing::should_use_hyprnote_routing;
use crate::query_params::QueryParams;

use super::{AppState, MAX_BATCH_AUDIO_BODY_BYTES};

pub async fn handler(
    State(state): State<AppState>,
    auth: Option<axum::Extension<AuthContext>>,
    headers: HeaderMap,
    mut params: QueryParams,
    body: Body,
) -> Response {
    if params.get_first("callback").is_some() {
        let body = match axum::body::to_bytes(body, MAX_BATCH_AUDIO_BODY_BYTES).await {
            Ok(body) => body,
            Err(error) => return body_read_error_response(error),
        };

        if body.is_empty() {
            return missing_audio_response();
        }

        return async_callback::handle_callback(&state, auth, &mut params, body)
            .await
            .into_response();
    }

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
    let use_hyprnote_routing = should_use_hyprnote_routing(provider_param.as_deref());

    if use_hyprnote_routing {
        return sync::handle_hyprnote_batch(
            &state,
            &params,
            listen_params,
            audio.path(),
            audio.len(),
            content_type,
        )
        .await;
    }

    let selected = match state.resolve_provider(&mut params) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    tracing::info!(
        hyprnote.stt.provider.name = ?selected.provider(),
        hyprnote.file.mime_type = %content_type,
        hyprnote.payload.size_bytes = %audio.len(),
        "batch_transcription_request_received"
    );

    let retry_config = state
        .router
        .as_ref()
        .map(|r| r.retry_config().clone())
        .unwrap_or_default();

    match sync::transcribe_with_retry(&selected, listen_params, audio.path(), &retry_config).await {
        Ok((response, _retries)) => Json(response).into_response(),
        Err((e, _retries)) => {
            tracing::error!(
                error = %e,
                hyprnote.stt.provider.name = ?selected.provider(),
                "batch_transcription_failed"
            );
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
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({
            "error": "invalid_request_body",
            "detail": error.to_string()
        })),
    )
        .into_response()
}

pub(super) fn build_listen_params(params: &QueryParams) -> ListenParams {
    normalize_listen_params(ListenParams {
        model: params.get_first("model").map(|s| s.to_string()),
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

pub(super) fn write_bytes_to_temp_file(
    bytes: &Bytes,
    content_type: &str,
) -> Result<tempfile::NamedTempFile, io::Error> {
    let extension = content_type_to_extension(content_type);
    let mut temp_file = tempfile::Builder::new()
        .prefix("batch_audio_")
        .suffix(&format!(".{}", extension))
        .tempfile()?;

    temp_file.write_all(bytes)?;
    temp_file.flush()?;

    Ok(temp_file)
}

async fn write_body_to_temp_file(
    body: Body,
    content_type: &str,
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
        if len > MAX_BATCH_AUDIO_BODY_BYTES as u64 {
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
    use hypr_language::ISO639;

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
}
