use std::path::PathBuf;

use hypr_local_model::LocalModel;
use hypr_model_downloader::ModelDownloaderRuntime;
use indicatif::ProgressBar;

pub(super) struct CliModelRuntime {
    pub(super) models_base: PathBuf,
    pub(super) progress: Option<ProgressBar>,
}

impl ModelDownloaderRuntime<LocalModel> for CliModelRuntime {
    fn models_base(&self) -> Result<PathBuf, hypr_model_downloader::Error> {
        Ok(self.models_base.clone())
    }

    fn emit_progress(&self, model: &LocalModel, progress: i8) {
        let Some(progress_bar) = &self.progress else {
            return;
        };

        if progress < 0 {
            progress_bar.abandon_with_message(format!("{}: failed", model.cli_name()));
            return;
        }

        if progress >= 0 {
            progress_bar.set_message(format!("Downloading {}", model.cli_name()));
            progress_bar.set_position((progress as u64).min(100));
        }
    }
}
