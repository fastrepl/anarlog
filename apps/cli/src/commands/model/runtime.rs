use std::path::PathBuf;

use hypr_local_model::LocalModel;
use hypr_model_downloader::ModelDownloaderRuntime;
use tokio::sync::mpsc;

pub(crate) enum DownloadEvent {
    Progress(u8),
    Completed,
    Failed,
}

pub(crate) struct CliModelRuntime {
    pub(crate) models_base: PathBuf,
    pub(crate) progress_tx: Option<mpsc::UnboundedSender<DownloadEvent>>,
}

impl ModelDownloaderRuntime<LocalModel> for CliModelRuntime {
    fn models_base(&self) -> Result<PathBuf, hypr_model_downloader::Error> {
        Ok(self.models_base.clone())
    }

    fn emit_progress(&self, _model: &LocalModel, status: hypr_model_downloader::DownloadStatus) {
        let Some(tx) = &self.progress_tx else {
            return;
        };

        use hypr_model_downloader::DownloadStatus;
        match status {
            DownloadStatus::Downloading(p) => {
                let _ = tx.send(DownloadEvent::Progress(p));
            }
            DownloadStatus::Completed => {
                let _ = tx.send(DownloadEvent::Completed);
            }
            DownloadStatus::Failed => {
                let _ = tx.send(DownloadEvent::Failed);
            }
        }
    }
}
