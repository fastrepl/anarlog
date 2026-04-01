use tokio::fs;

use crate::Error;
use crate::download_task::params::DownloadTaskParams;
use crate::model::DownloadableModel;

pub(super) enum ChecksumError {
    Mismatch { actual: u32, expected: u32 },
    Calculate(hypr_file::Error),
    Join(tokio::task::JoinError),
}

pub(super) enum FinalizeError {
    Finalize(Error),
    Join(tokio::task::JoinError),
}

pub(super) async fn download<M: DownloadableModel>(
    params: &DownloadTaskParams<M>,
    progress_callback: impl Fn(hypr_download_interface::DownloadProgress) + Send + Sync,
) -> Result<(), hypr_file::Error> {
    hypr_file::download_file_parallel_cancellable(
        &params.url,
        &params.destination,
        progress_callback,
        Some(params.cancellation_token.clone()),
    )
    .await
}

pub(super) async fn verify_checksum<M: DownloadableModel>(
    params: &DownloadTaskParams<M>,
    expected_checksum: u32,
) -> Result<(), ChecksumError> {
    let destination_for_checksum = params.destination.clone();
    let checksum_result = tokio::task::spawn_blocking(move || {
        hypr_file::calculate_file_checksum(destination_for_checksum)
    })
    .await;

    match checksum_result {
        Ok(Ok(actual_checksum)) => {
            if actual_checksum == expected_checksum {
                Ok(())
            } else {
                Err(ChecksumError::Mismatch {
                    actual: actual_checksum,
                    expected: expected_checksum,
                })
            }
        }
        Ok(Err(e)) => Err(ChecksumError::Calculate(e)),
        Err(e) => Err(ChecksumError::Join(e)),
    }
}

pub(super) async fn finalize<M: DownloadableModel>(
    params: &DownloadTaskParams<M>,
) -> Result<(), FinalizeError> {
    let destination_for_finalize = params.destination.clone();
    let model_for_finalize = params.model.clone();
    let models_base_for_finalize = params.models_base.clone();
    let finalize_result = tokio::task::spawn_blocking(move || {
        model_for_finalize.finalize_download(&destination_for_finalize, &models_base_for_finalize)
    })
    .await;

    match finalize_result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(FinalizeError::Finalize(e)),
        Err(e) => Err(FinalizeError::Join(e)),
    }
}

pub(super) async fn promote<M: DownloadableModel>(
    params: &DownloadTaskParams<M>,
) -> Result<(), std::io::Error> {
    match fs::rename(&params.destination, &params.final_destination).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&params.final_destination).await;
            fs::rename(&params.destination, &params.final_destination).await
        }
        Err(e) => Err(e),
    }
}
