use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub(crate) struct DownloadsRegistry {
    inner: Arc<Mutex<HashMap<String, DownloadEntry>>>,
}

pub(crate) struct DownloadEntry {
    pub(crate) task: JoinHandle<()>,
    pub(crate) token: CancellationToken,
    pub(crate) generation: u64,
    pub(crate) download_path: PathBuf,
}

impl DownloadsRegistry {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) async fn contains(&self, key: &str) -> bool {
        self.inner.lock().await.contains_key(key)
    }

    pub(crate) async fn insert(&self, key: String, entry: DownloadEntry) -> Option<DownloadEntry> {
        self.inner.lock().await.insert(key, entry)
    }

    pub(crate) async fn remove(&self, key: &str) -> Option<DownloadEntry> {
        self.inner.lock().await.remove(key)
    }

    pub(crate) async fn remove_if_generation_matches(&self, key: &str, generation: u64) {
        let mut guard = self.inner.lock().await;
        if guard
            .get(key)
            .is_some_and(|entry| entry.generation == generation)
        {
            guard.remove(key);
        }
    }
}

impl Clone for DownloadsRegistry {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
