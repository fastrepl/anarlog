use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use owhisper_client::{CallbackResult, CallbackSttAdapter};
use serde::Deserialize;

use super::{AppState, RouteError, parse_async_provider};
use crate::supabase::{JobUpdate, PipelineStatus, SupabaseClient};

#[derive(Deserialize)]
pub(crate) struct CallbackQuery {
    secret: Option<String>,
}

pub async fn handler(
    State(state): State<AppState>,
    supabase: SupabaseClient,
    Path((provider, id)): Path<(String, String)>,
    Query(query): Query<CallbackQuery>,
    body: axum::body::Bytes,
) -> Result<StatusCode, RouteError> {
    let expected_secret = state
        .config
        .callback
        .secret
        .as_deref()
        .ok_or(RouteError::MissingConfig("callback_secret not configured"))?;

    match query.secret.as_deref() {
        Some(s) if s == expected_secret => {}
        _ => return Err(RouteError::Unauthorized("invalid callback secret")),
    }

    let payload: serde_json::Value = serde_json::from_slice(&body).map_err(|e| {
        tracing::warn!(error = %e, "invalid callback payload");
        RouteError::BadRequest("invalid JSON payload".into())
    })?;

    let owhisper_provider = parse_async_provider(&provider)?;

    let api_key = state
        .config
        .api_keys
        .get(&owhisper_provider)
        .cloned()
        .ok_or(RouteError::MissingConfig(
            "api_key not configured for provider",
        ))?;

    let outcome = match owhisper_provider {
        owhisper_client::Provider::Soniox => {
            owhisper_client::SonioxAdapter
                .process_callback(&state.client, &api_key, payload)
                .await
        }
        owhisper_client::Provider::Deepgram => {
            owhisper_client::DeepgramAdapter
                .process_callback(&state.client, &api_key, payload)
                .await
        }
        _ => unreachable!(),
    }
    .map_err(|e| {
        tracing::error!(
            hyprnote.stt.job.id = %id,
            hyprnote.stt.provider.name = %provider,
            error = %e,
            "callback processing failed"
        );
        RouteError::Internal(format!("callback processing failed: {e}"))
    })?;

    let update = match &outcome {
        CallbackResult::Done(raw_result) => JobUpdate {
            status: PipelineStatus::Done,
            raw_result: Some(raw_result.clone()),
            error: None,
        },
        CallbackResult::ProviderError(message) => JobUpdate {
            status: PipelineStatus::Error,
            raw_result: None,
            error: Some(message.clone()),
        },
    };

    supabase
        .update_job(&id, &update)
        .await
        .map_err(|e| RouteError::Internal(format!("failed to update job: {e}")))?;

    cleanup_audio(&supabase, &id).await;

    Ok(StatusCode::OK)
}

async fn cleanup_audio(supabase: &SupabaseClient, job_id: &str) {
    let job = match supabase.get_job(job_id).await {
        Ok(Some(j)) => j,
        Ok(None) => return,
        Err(e) => {
            tracing::warn!(
                hyprnote.stt.job.id = %job_id,
                error = %e,
                "failed to fetch job for cleanup"
            );
            return;
        }
    };

    if let Err(e) = supabase
        .storage()
        .delete_file("audio-files", &job.file_id)
        .await
    {
        tracing::warn!(
            hyprnote.stt.job.id = %job_id,
            hyprnote.file.id = %job.file_id,
            error = %e,
            "failed to delete audio file"
        );
    }
}
