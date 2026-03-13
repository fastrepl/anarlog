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
            let emit_failure = log_download_error(&error);
            fail_task(&params, emit_failure).await;
            return;
        }

        if let Some(expected_checksum) = params.model.download_checksum()
            && let Err(error) = steps::verify_checksum(&params, expected_checksum).await
        {
            log_checksum_error(&error);
            fail_task(&params, true).await;
            return;
        }

        if let Err(error) = steps::finalize(&params).await {
            log_finalize_error(&error);
            fail_task(&params, true).await;
            return;
        }

        if params.model.remove_destination_after_finalize() {
            let _ = tokio::fs::remove_file(&params.destination).await;
        } else if let Err(error) = steps::promote(&params).await {
            tracing::error!(error = %error, "model_download_promote_error");
            fail_task(&params, true).await;
            return;
        }

        params.runtime.emit_progress(&params.model, 100);
        params
            .registry
            .remove_if_generation_matches(&params.key, params.generation)
            .await;
    })
}

async fn fail_task<M: DownloadableModel>(params: &DownloadTaskParams<M>, emit_failure: bool) {
    if emit_failure {
        params.runtime.emit_progress(&params.model, -1);
    }
    cleanup_for_failure(params).await;
}

fn log_download_error(error: &hypr_file::Error) -> bool {
    if matches!(error, hypr_file::Error::Cancelled) {
        return false;
    }

    tracing::error!(error = %error, "model_download_error");
    true
}

fn log_checksum_error(error: &ChecksumError) {
    match error {
        ChecksumError::Mismatch { actual, expected } => {
            tracing::error!(
                actual_checksum = actual,
                expected_checksum = expected,
                "model_download_checksum_mismatch"
            );
        }
        ChecksumError::Calculate(error) => {
            tracing::error!(error = %error, "model_download_checksum_error");
        }
        ChecksumError::Join(error) => {
            tracing::error!(error = %error, "model_download_checksum_join_error");
        }
    }
}

fn log_finalize_error(error: &FinalizeError) {
    match error {
        FinalizeError::Finalize(error) => {
            tracing::error!(error = %error, "model_finalize_error");
        }
        FinalizeError::Join(error) => {
            tracing::error!(error = %error, "model_finalize_join_error");
        }
    }
}
