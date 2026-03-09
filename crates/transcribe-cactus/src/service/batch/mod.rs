mod chunk;
mod response;
mod transcribe;

use std::convert::Infallible;
use std::path::Path;

use axum::{
    Json,
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
};
use bytes::Bytes;
use owhisper_interface::ListenParams;
use owhisper_interface::batch_sse::{BatchSseMessage, EVENT_NAME};
use tokio::sync::mpsc;

use transcribe::transcribe_batch;

pub async fn handle_batch(
    body: Bytes,
    content_type: &str,
    params: &ListenParams,
    model_path: &Path,
) -> Response {
    let model_path = model_path.to_path_buf();
    let content_type = content_type.to_string();
    let params = params.clone();

    let result = tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transcribe_batch(&body, &content_type, &params, &model_path, None)
        }))
    })
    .await;

    match result {
        Ok(Ok(Ok(response))) => Json(response).into_response(),
        Ok(Ok(Err(e))) => {
            tracing::error!(error.message = %e, "batch_transcription_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "transcription_failed",
                    "detail": e.to_string()
                })),
            )
                .into_response()
        }
        Ok(Err(_)) | Err(_) => {
            tracing::error!("batch_task_panicked");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "transcription_failed",
                    "detail": "task panicked"
                })),
            )
                .into_response()
        }
    }
}

pub async fn handle_batch_sse(
    body: Bytes,
    content_type: &str,
    params: &ListenParams,
    model_path: &Path,
) -> Response {
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
                &model_path,
                Some(event_tx.clone()),
            )
        })) {
            Ok(Ok(response)) => BatchSseMessage::Result { response },
            Ok(Err(e)) => {
                tracing::error!(error.message = %e, "batch_sse transcription failed");
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

    let events_stream = futures_util::stream::unfold(event_rx, |mut rx| async move {
        rx.recv().await.map(|message| {
            let event = match Event::default().event(EVENT_NAME).json_data(&message) {
                Ok(event) => event,
                Err(error) => {
                    tracing::warn!("failed to serialize batch SSE event: {error}");
                    Event::default()
                        .event(EVENT_NAME)
                        .data(r#"{"error":"transcription_failed","detail":"failed to serialize SSE event"}"#)
                }
            };
            (Ok::<_, Infallible>(event), rx)
        })
    });

    Sse::new(events_stream).into_response()
}
