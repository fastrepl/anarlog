use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use tokio::sync::{Mutex, RwLock, watch};

use crate::{ModelLoader, ModelManager};

const DEFAULT_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_CHECK_INTERVAL: Duration = Duration::from_secs(3);

pub(crate) struct DropGuard {
    shutdown_tx: watch::Sender<()>,
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
    }
}

pub struct ModelManagerBuilder<M: ModelLoader> {
    models: HashMap<String, PathBuf>,
    default_model: Option<String>,
    inactivity_timeout: Option<Duration>,
    check_interval: Option<Duration>,
    _phantom: std::marker::PhantomData<M>,
}

impl<M: ModelLoader> Default for ModelManagerBuilder<M> {
    fn default() -> Self {
        Self {
            models: HashMap::new(),
            default_model: None,
            inactivity_timeout: None,
            check_interval: None,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<M: ModelLoader> ModelManagerBuilder<M> {
    pub fn register(mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        self.models.insert(name.into(), path.into());
        self
    }

    pub fn default_model(mut self, name: impl Into<String>) -> Self {
        self.default_model = Some(name.into());
        self
    }

    pub fn inactivity_timeout(mut self, timeout: Duration) -> Self {
        self.inactivity_timeout = Some(timeout);
        self
    }

    pub fn check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = Some(interval);
        self
    }

    pub fn build(self) -> ModelManager<M> {
        let (shutdown_tx, shutdown_rx) = watch::channel(());
        let inactivity_timeout = self
            .inactivity_timeout
            .unwrap_or(DEFAULT_INACTIVITY_TIMEOUT);
        let check_interval = self.check_interval.unwrap_or(DEFAULT_CHECK_INTERVAL);

        let manager = ModelManager {
            registry: Arc::new(RwLock::new(self.models)),
            default_model: Arc::new(RwLock::new(self.default_model)),
            active: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(Mutex::new(None)),
            inactivity_timeout,
            _drop_guard: Arc::new(DropGuard { shutdown_tx }),
        };

        manager.spawn_monitor(check_interval, shutdown_rx);
        manager
    }
}
