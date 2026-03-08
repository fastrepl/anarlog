pub mod async_callback;
mod sync;

use std::io::Write;

use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use hypr_api_auth::AuthContext;
use owhisper_interface::ListenParams;

use hypr_audio_mime::content_type_to_extension;

use crate::hyprnote_routing::should_use_hyprnote_routing;
use crate::query_params::QueryParams;

use super::AppState;

pub async fn handler(
    State(state): State<AppState>,
    auth: Option<axum::Extension<AuthContext>>,
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
        return sync::handle_hyprnote_batch(&state, &params, listen_params, body, content_type)
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

    match sync::transcribe_with_provider(&selected, listen_params, body, content_type).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => {
            tracing::error!(
                error.message = %e,
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
    ListenParams {
        model: params.get_first("model").map(|s| s.to_string()),
        languages: params.get_languages(),
        keywords: params.parse_keywords(),
        ..Default::default()
    }
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
