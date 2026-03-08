use std::path::PathBuf;

use crate::model::DownloadableModel;

pub trait ModelDownloaderRuntime<M: DownloadableModel>: Send + Sync + 'static {
    fn models_base(&self) -> Result<PathBuf, crate::Error>;
    fn emit_progress(&self, model: &M, progress: i8);
}
