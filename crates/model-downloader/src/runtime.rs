use std::path::PathBuf;

use crate::model::DownloadableModel;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum DownloadStatus {
    Downloading(u8),
    Completed,
    Failed,
}

pub trait ModelDownloaderRuntime<M: DownloadableModel>: Send + Sync + 'static {
    fn models_base(&self) -> Result<PathBuf, crate::Error>;
    fn emit_progress(&self, model: &M, status: DownloadStatus);
}
