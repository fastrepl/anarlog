use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::download_task::failure::cleanup_for_failure;
use crate::download_task::steps::{ChecksumError, FinalizeError};
use crate::download_task_progress::make_progress_callback;
use crate::model::DownloadableModel;

mod failure;
mod params;
mod steps;

pub(crate) use params::DownloadTaskParams;

pub(crate) fn spawn_download_task<M: DownloadableModel>(
    params: DownloadTaskParams<M>,
    start_rx: oneshot::Receiver<()>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if start_rx.await.is_err() {
            cleanup_for_failure(&params).await;
            return;
        }

        let progress_callback =
            make_progress_callback(params.runtime.clone(), params.model.clone());

        if let Err(error) = steps::download(&params, progress_callback).await {
            let reason = log_download_error(&error);
            fail_task(&params, reason).await;
            return;
        }

        if let Some(expected_checksum) = params.model.download_checksum()
            && let Err(error) = steps::verify_checksum(&params, expected_checksum).await
        {
            let reason = log_checksum_error(&error);
            fail_task(&params, Some(reason)).await;
            return;
        }

        if let Err(error) = steps::finalize(&params).await {
            let reason = log_finalize_error(&error);
            fail_task(&params, Some(reason)).await;
            return;
        }

        if params.model.remove_destination_after_finalize() {
            let _ = tokio::fs::remove_file(&params.destination).await;
        } else if let Err(error) = steps::promote(&params).await {
            tracing::error!(error = %error, "model_download_promote_error");
            let reason = format!("Failed to move model file: {}", error);
            fail_task(&params, Some(reason)).await;
            return;
        }

        params
            .runtime
            .emit_progress(&params.model, crate::runtime::DownloadStatus::Completed);
        params
            .registry
            .remove_if_generation_matches(&params.key, params.generation)
            .await;
    })
}

async fn fail_task<M: DownloadableModel>(params: &DownloadTaskParams<M>, reason: Option<String>) {
    if let Some(reason) = reason {
        params.runtime.emit_progress(
            &params.model,
            crate::runtime::DownloadStatus::Failed(reason),
        );
    }
    cleanup_for_failure(params).await;
}

fn log_download_error(error: &hypr_file::Error) -> Option<String> {
    if matches!(error, hypr_file::Error::Cancelled) {
        return None;
    }

    tracing::error!(error = %error, "model_download_error");

    let reason = match error {
        hypr_file::Error::ReqwestError(e) => {
            if e.is_timeout() {
                "Download timed out. Please check your internet connection and try again."
                    .to_string()
            } else if e.is_connect() {
                "Could not connect to the download server. Please check your internet connection."
                    .to_string()
            } else {
                format!("Network error: {}", e)
            }
        }
        hypr_file::Error::FileIOError(e) => {
            format!("File system error: {}", e)
        }
        hypr_file::Error::Cancelled => unreachable!(),
        hypr_file::Error::OtherError(msg) => msg.clone(),
    };
    Some(reason)
}

fn log_checksum_error(error: &ChecksumError) -> String {
    match error {
        ChecksumError::Mismatch { actual, expected } => {
            tracing::error!(
                actual_checksum = actual,
                expected_checksum = expected,
                "model_download_checksum_mismatch"
            );
            "Downloaded file is corrupted (checksum mismatch). Please try again.".to_string()
        }
        ChecksumError::Calculate(error) => {
            tracing::error!(error = %error, "model_download_checksum_error");
            format!("Failed to verify download: {}", error)
        }
        ChecksumError::Join(error) => {
            tracing::error!(error = %error, "model_download_checksum_join_error");
            format!("Verification interrupted: {}", error)
        }
    }
}

fn log_finalize_error(error: &FinalizeError) -> String {
    match error {
        FinalizeError::Finalize(error) => {
            tracing::error!(error = %error, "model_finalize_error");
            format!("Failed to finalize model: {}", error)
        }
        FinalizeError::Join(error) => {
            tracing::error!(error = %error, "model_finalize_join_error");
            format!("Finalization interrupted: {}", error)
        }
    }
}
