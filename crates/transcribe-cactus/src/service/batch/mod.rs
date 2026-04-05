mod response;
mod transcribe;

use std::path::Path;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use tokio::sync::mpsc;

use hypr_model_manager::ModelManager;
use hypr_transcribe_core::{batch_sse_response, json_error_response};
use owhisper_interface::ListenParams;
use owhisper_interface::batch_sse::BatchSseMessage;

use transcribe::transcribe_batch;

pub async fn handle_batch(
    body: Bytes,
    content_type: &str,
    params: &ListenParams,
    manager: &ModelManager<hypr_cactus::Model>,
    model_path: &Path,
) -> Response {
    let model = match manager.get(None).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error = %e, "failed_to_load_model");
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model_load_failed",
                e.to_string(),
            );
        }
    };

    let model_path = model_path.to_path_buf();
    let content_type = content_type.to_string();
    let params = params.clone();

    let result = tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transcribe_batch(&body, &content_type, &params, &model, &model_path, None)
        }))
    })
    .await;

    match result {
        Ok(Ok(Ok(response))) => Json(response).into_response(),
        Ok(Ok(Err(e))) => {
            tracing::error!(error = %e, "batch_transcription_failed");
            json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "transcription_failed",
                e.to_string(),
            )
        }
        Ok(Err(_)) | Err(_) => {
            tracing::error!("batch_task_panicked");
            json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "transcription_failed",
                "task panicked",
            )
        }
    }
}

pub async fn handle_batch_sse(
    body: Bytes,
    content_type: &str,
    params: &ListenParams,
    manager: &ModelManager<hypr_cactus::Model>,
    model_path: &Path,
) -> Response {
    let model = match manager.get(None).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error = %e, "failed_to_load_model");
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model_load_failed",
                e.to_string(),
            );
        }
    };

    let model_path = model_path.to_path_buf();
    let content_type = content_type.to_string();
    let params = params.clone();

    let (event_tx, event_rx) = mpsc::unbounded_channel::<BatchSseMessage>();

    tokio::task::spawn_blocking(move || {
        let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transcribe_batch(
                &body,
                &content_type,
                &params,
                &model,
                &model_path,
                Some(event_tx.clone()),
            )
        })) {
            Ok(Ok(response)) => BatchSseMessage::Result { response },
            Ok(Err(e)) => {
                tracing::error!(error = %e, "batch_sse transcription failed");
                BatchSseMessage::Error {
                    error: "transcription_failed".to_string(),
                    detail: e.to_string(),
                }
            }
            Err(_) => {
                tracing::error!("batch_sse transcription task panicked");
                BatchSseMessage::Error {
                    error: "transcription_failed".to_string(),
                    detail: "task panicked".to_string(),
                }
            }
        };

        let _ = event_tx.send(message);
    });

    batch_sse_response(event_rx)
}
