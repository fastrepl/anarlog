use axum::{Json, body::Bytes};
use hypr_api_auth::AuthContext;
use owhisper_client::{CallbackSttAdapter, DeepgramAdapter, Provider, SonioxAdapter};
use owhisper_interface::ListenParams;
use serde::{Deserialize, Serialize};

use crate::query_params::QueryParams;
use crate::supabase::{PipelineStatus, SupabaseClient, TranscriptionJob};

use super::super::{AppState, RouteError, parse_async_provider};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ListenCallbackRequest {
    pub url: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ListenCallbackResponse {
    pub request_id: String,
}

pub(super) async fn handle_callback(
    state: &AppState,
    auth: Option<axum::Extension<AuthContext>>,
    params: &mut QueryParams,
    body: Bytes,
) -> Result<Json<ListenCallbackResponse>, RouteError> {
    let auth = auth.ok_or(RouteError::Unauthorized("authentication required"))?;
    let user_id = auth.claims.sub.clone();

    let supabase = build_supabase_client(state)?;

    let provider_str = params
        .remove_first("provider")
        .unwrap_or_else(|| "deepgram".to_string());
    let provider = parse_async_provider(&provider_str)?;
    let listen_params = super::build_listen_params(params);

    let id = uuid::Uuid::new_v4().to_string();

    let req: ListenCallbackRequest = serde_json::from_slice(&body)
        .map_err(|_| RouteError::BadRequest("expected JSON body with url field".into()))?;
    let file_id = req.url;

    let audio_url = supabase
        .storage()
        .create_signed_url("audio-files", &file_id, 3600)
        .await
        .map_err(|e| {
            tracing::error!(
                hyprnote.file.id = %file_id,
                error.message = %e,
                "failed to create signed URL"
            );
            RouteError::Internal(format!("failed to create signed URL: {e}"))
        })?;

    let is_local =
        audio_url.starts_with("http://127.0.0.1") || audio_url.starts_with("http://localhost");

    let (status, provider_request_id, raw_result, error) = if is_local {
        handle_sync_fallback(
            state,
            &provider_str,
            provider,
            &listen_params,
            &audio_url,
            &file_id,
        )
        .await?
    } else {
        let provider_request_id =
            handle_remote_callback(state, &provider_str, provider, &audio_url, &id).await?;
        (
            PipelineStatus::Processing,
            Some(provider_request_id),
            None,
            None,
        )
    };

    let job = TranscriptionJob {
        id: id.clone(),
        user_id,
        file_id,
        provider: provider_str.to_string(),
        status,
        provider_request_id,
        raw_result,
        error,
    };

    supabase.insert_job(&job).await.map_err(|e| {
        tracing::error!(
            hyprnote.stt.job.id = %id,
            error.message = %e,
            "failed to insert job"
        );
        RouteError::Internal(format!("failed to record job: {e}"))
    })?;

    Ok(Json(ListenCallbackResponse { request_id: id }))
}

async fn handle_sync_fallback(
    state: &AppState,
    provider_str: &str,
    provider: Provider,
    listen_params: &ListenParams,
    audio_url: &str,
    file_id: &str,
) -> Result<
    (
        PipelineStatus,
        Option<String>,
        Option<serde_json::Value>,
        Option<String>,
    ),
    RouteError,
> {
    tracing::info!(
        hyprnote.stt.provider.name = %provider_str,
        "local_url_detected, using sync transcription"
    );

    let download_response =
        hypr_observability::with_current_trace_context(state.client.get(audio_url))
            .send()
            .await
            .map_err(|e| RouteError::Internal(format!("failed to download audio: {e}")))?;

    let download_status = download_response.status();
    let audio_bytes = download_response
        .bytes()
        .await
        .map_err(|e| RouteError::Internal(format!("failed to read audio bytes: {e}")))?;

    if !download_status.is_success() || audio_bytes.len() < 1024 {
        let body_preview = String::from_utf8_lossy(&audio_bytes[..audio_bytes.len().min(512)]);
        tracing::error!(
            http.response.status_code = %download_status.as_u16(),
            hyprnote.audio.size_bytes = audio_bytes.len(),
            hyprnote.http.body_preview = %body_preview,
            url.full = %audio_url,
            "signed_url_download_failed"
        );
        if !download_status.is_success() {
            return Err(RouteError::Internal(format!(
                "failed to download audio from storage: {download_status}"
            )));
        }
    }

    let content_type = content_type_from_filename(file_id);

    tracing::info!(
        hyprnote.file.mime_type = %content_type,
        hyprnote.audio.size_bytes = audio_bytes.len(),
        hyprnote.file.id = %file_id,
        "sync_fallback_audio_downloaded"
    );

    let selected = state
        .config
        .provider_selector()
        .select(Some(provider))
        .map_err(|_| RouteError::MissingConfig("api_key not configured for provider"))?;

    match super::sync::transcribe_with_provider(
        &selected,
        listen_params.clone(),
        audio_bytes,
        content_type,
    )
    .await
    {
        Ok(response) => {
            let raw_result = serde_json::to_value(&response)
                .map_err(|e| RouteError::Internal(format!("failed to serialize result: {e}")))?;
            Ok((PipelineStatus::Done, None, Some(raw_result), None))
        }
        Err(e) => {
            tracing::error!(
                error.message = %e,
                hyprnote.stt.provider.name = %provider_str,
                "sync transcription failed"
            );
            Ok((
                PipelineStatus::Error,
                None,
                None,
                Some(e.message().to_string()),
            ))
        }
    }
}

async fn handle_remote_callback(
    state: &AppState,
    provider_str: &str,
    provider: Provider,
    audio_url: &str,
    id: &str,
) -> Result<String, RouteError> {
    let api_base_url = state
        .config
        .callback
        .api_base_url
        .as_deref()
        .ok_or(RouteError::MissingConfig("api_base_url not configured"))?
        .trim_end_matches('/');

    let callback_secret = state
        .config
        .callback
        .secret
        .as_deref()
        .ok_or(RouteError::MissingConfig("callback_secret not configured"))?;

    let callback_url =
        format!("{api_base_url}/stt/callback/{provider_str}/{id}?secret={callback_secret}");

    let api_key = state
        .config
        .api_keys
        .get(&provider)
        .ok_or(RouteError::MissingConfig(
            "api_key not configured for provider",
        ))?;

    match provider {
        Provider::Soniox => {
            SonioxAdapter
                .submit_callback(&state.client, api_key, audio_url, &callback_url)
                .await
        }
        Provider::Deepgram => {
            DeepgramAdapter
                .submit_callback(&state.client, api_key, audio_url, &callback_url)
                .await
        }
        _ => unreachable!(),
    }
    .map_err(|e| {
        tracing::error!(
            error.message = %e,
            hyprnote.stt.provider.name = %provider_str,
            "submission failed"
        );
        RouteError::BadGateway(format!("{provider_str} submission failed: {e}"))
    })
}

fn content_type_from_filename(file_id: &str) -> &'static str {
    let ext = file_id.rsplit('.').next().unwrap_or("");
    match ext {
        "wav" | "wave" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" | "mp4" => "audio/mp4",
        "webm" => "audio/webm",
        "aac" => "audio/aac",
        _ => "application/octet-stream",
    }
}

fn build_supabase_client(state: &AppState) -> Result<SupabaseClient, RouteError> {
    let url = state
        .config
        .supabase
        .url
        .as_deref()
        .ok_or(RouteError::MissingConfig("supabase_url not configured"))?;
    let key =
        state
            .config
            .supabase
            .service_role_key
            .as_deref()
            .ok_or(RouteError::MissingConfig(
                "supabase_service_role_key not configured",
            ))?;
    Ok(SupabaseClient::new(state.client.clone(), url, key))
}
