use std::path::PathBuf;
use tokio::sync::RwLock;

use crate::Error;
use crate::PriorityState;
use crate::ext::FILENAME;

pub struct AudioPriorityState {
    path: PathBuf,
    lock: RwLock<()>,
}

impl AudioPriorityState {
    pub fn new(base: PathBuf) -> Self {
        let path = base.join(FILENAME);
        Self {
            path,
            lock: RwLock::new(()),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    async fn read_or_default(&self) -> crate::Result<PriorityState> {
        match tokio::fs::read_to_string(&self.path).await {
            Ok(content) => {
                serde_json::from_str(&content).map_err(|e| Error::State(format!("parse: {}", e)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PriorityState::default()),
            Err(e) => Err(Error::State(format!("read: {}", e))),
        }
    }

    pub async fn load(&self) -> crate::Result<PriorityState> {
        let _guard = self.lock.read().await;
        self.read_or_default().await
    }

    pub async fn save(&self, state: PriorityState) -> crate::Result<()> {
        let _guard = self.lock.write().await;

        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::State(format!("create dir: {}", e)))?;
        }

        let tmp_path = self.path.with_extension("for-save.tmp");
        let content = serde_json::to_string_pretty(&state)?;

        tokio::fs::write(&tmp_path, &content).await?;
        tokio::fs::rename(&tmp_path, &self.path).await?;

        tracing::debug!("Saved audio priority state to {:?}", self.path);
        Ok(())
    }

    pub fn reset(&self) -> crate::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let tmp_path = self.path.with_extension("for-reset.tmp");
        let content = serde_json::to_string_pretty(&PriorityState::default())?;
        std::fs::write(&tmp_path, &content)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}
