use std::path::PathBuf;
use std::sync::Arc;

use hf_hub::api::tokio::{ApiBuilder, Progress};
use hypr_download_interface::DownloadProgress;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Api(#[from] hf_hub::api::tokio::ApiError),
}

pub async fn download<F>(repo_id: &str, filename: &str, callback: F) -> Result<PathBuf, Error>
where
    F: Fn(DownloadProgress) + Send + Sync + 'static,
{
    let api = ApiBuilder::new().with_progress(false).build()?;
    let bridge = ProgressBridge {
        callback: Arc::new(callback),
        total: 0,
        current: 0,
    };
    let path = api
        .model(repo_id.to_string())
        .download_with_progress(filename, bridge)
        .await?;
    Ok(path)
}

struct ProgressBridge<F> {
    callback: Arc<F>,
    total: u64,
    current: u64,
}

impl<F> Clone for ProgressBridge<F> {
    fn clone(&self) -> Self {
        Self {
            callback: Arc::clone(&self.callback),
            total: self.total,
            current: self.current,
        }
    }
}

impl<F: Fn(DownloadProgress) + Send + Sync> Progress for ProgressBridge<F> {
    async fn init(&mut self, size: usize, _filename: &str) {
        self.total = size as u64;
        self.current = 0;
        (self.callback)(DownloadProgress::Started);
    }

    async fn update(&mut self, size: usize) {
        self.current += size as u64;
        (self.callback)(DownloadProgress::Progress(self.current, self.total));
    }

    async fn finish(&mut self) {
        (self.callback)(DownloadProgress::Finished);
    }
}
