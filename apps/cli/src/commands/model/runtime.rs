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

    fn emit_progress(&self, _model: &LocalModel, progress: i8) {
        let Some(tx) = &self.progress_tx else {
            return;
        };

        if progress < 0 {
            let _ = tx.send(DownloadEvent::Failed);
        } else if progress >= 100 {
            let _ = tx.send(DownloadEvent::Completed);
        } else {
            let _ = tx.send(DownloadEvent::Progress(progress as u8));
        }
    }
}
