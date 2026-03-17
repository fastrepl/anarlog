use tokio::fs;

use crate::download_task::params::DownloadTaskParams;
use crate::model::DownloadableModel;

pub(super) async fn cleanup_for_failure<M: DownloadableModel>(params: &DownloadTaskParams<M>) {
    let _ = fs::remove_file(&params.destination).await;
    params
        .registry
        .remove_if_generation_matches(&params.key, params.generation)
        .await;
}
