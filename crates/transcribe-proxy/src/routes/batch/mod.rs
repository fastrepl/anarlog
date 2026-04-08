pub mod async_callback;
mod sync;

use std::io::Write;

use axum::{
    Json,
    body::Bytes,
    extract::{FromRequestParts, State},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use hypr_api_auth::AuthContext;
use owhisper_client::normalize_listen_params;
use owhisper_interface::ListenParams;

use hypr_audio_mime::content_type_to_extension;

use crate::hyprnote_routing::should_use_hyprnote_routing;
use crate::query_params::QueryParams;

use super::AppState;

pub(crate) struct WantsBatchSse(pub bool);

impl<S> FromRequestParts<S> for WantsBatchSse
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .headers
                .get("accept")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.contains("text/event-stream")),
        ))
    }
}

pub async fn handler(
    State(state): State<AppState>,
    auth: Option<axum::Extension<AuthContext>>,
    WantsBatchSse(wants_batch_sse): WantsBatchSse,
    headers: HeaderMap,
    mut params: QueryParams,
    body: Bytes,
) -> Response {
    if body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "missing_audio_data",
                "detail": "Request body is empty"
            })),
        )
            .into_response();
    }

    if params.get_first("callback").is_some() {
        return async_callback::handle_callback(&state, auth, &mut params, body)
            .await
            .into_response();
    }

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");

    let listen_params = build_listen_params(&params);

    let provider_param = params.get_first("provider").map(|s| s.to_string());
    let use_hyprnote_routing = should_use_hyprnote_routing(provider_param.as_deref());

    if use_hyprnote_routing {
        if wants_batch_sse {
            return sync::handle_routed_batch_sse(
                &state,
                &params,
                listen_params,
                body,
                content_type,
            );
        }

        return sync::handle_routed_batch_json(&state, &params, listen_params, body, content_type)
            .await;
    }

    let selected = match state.resolve_provider(&mut params) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    tracing::info!(
        hyprnote.stt.provider.name = ?selected.provider(),
        hyprnote.file.mime_type = %content_type,
        hyprnote.payload.size_bytes = %body.len(),
        "batch_transcription_request_received"
    );

    let retry_config = state
        .router
        .as_ref()
        .map(|r| r.retry_config().clone())
        .unwrap_or_default();

    if wants_batch_sse {
        return sync::handle_direct_batch_sse(
            selected,
            listen_params,
            body,
            content_type,
            retry_config,
        );
    }

    match sync::transcribe_with_retry(&selected, listen_params, body, content_type, &retry_config)
        .await
    {
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

pub(super) fn build_listen_params(params: &QueryParams) -> ListenParams {
    normalize_listen_params(ListenParams {
        model: params.get_first("model").map(|s| s.to_string()),
        languages: params.get_languages(),
        keywords: params.parse_keywords(),
        ..Default::default()
    })
}

fn write_to_temp_file(
    bytes: &Bytes,
    content_type: &str,
) -> Result<tempfile::NamedTempFile, std::io::Error> {
    let extension = content_type_to_extension(content_type);
    let mut temp_file = tempfile::Builder::new()
        .prefix("batch_audio_")
        .suffix(&format!(".{}", extension))
        .tempfile()?;

    temp_file.write_all(bytes)?;
    temp_file.flush()?;

    Ok(temp_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query_params::QueryValue;
    use axum::http::Request;
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

    #[tokio::test]
    async fn wants_batch_sse_detects_event_stream_accept_header() {
        let request = Request::builder()
            .header("accept", "application/json, text/event-stream")
            .body(())
            .expect("request");
        let (mut parts, _) = request.into_parts();

        let wants_sse = WantsBatchSse::from_request_parts(&mut parts, &())
            .await
            .expect("extractor");

        assert!(wants_sse.0);
    }

    #[tokio::test]
    async fn wants_batch_sse_defaults_to_false() {
        let request = Request::builder().body(()).expect("request");
        let (mut parts, _) = request.into_parts();

        let wants_sse = WantsBatchSse::from_request_parts(&mut parts, &())
            .await
            .expect("extractor");

        assert!(!wants_sse.0);
    }
}
