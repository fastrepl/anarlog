use std::path::PathBuf;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::downloads_registry::DownloadsRegistry;
use crate::model::DownloadableModel;
use crate::runtime::ModelDownloaderRuntime;

pub(crate) struct DownloadTaskParams<M: DownloadableModel> {
    pub(crate) runtime: Arc<dyn ModelDownloaderRuntime<M>>,
    pub(crate) registry: DownloadsRegistry,
    pub(crate) model: M,
    pub(crate) url: String,
    pub(crate) destination: PathBuf,
    pub(crate) final_destination: PathBuf,
    pub(crate) models_base: PathBuf,
    pub(crate) key: String,
    pub(crate) generation: u64,
    pub(crate) cancellation_token: CancellationToken,
}
