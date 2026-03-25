use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use tokio::sync::{RwLock, watch};

use crate::loader::ModelLoader;
use crate::manager::{DEFAULT_CHECK_INTERVAL, DEFAULT_INACTIVITY_TIMEOUT, ModelManager};

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

        let manager = ModelManager::new(
            Arc::new(RwLock::new(self.models)),
            Arc::new(RwLock::new(self.default_model)),
            inactivity_timeout,
            shutdown_tx,
        );

        manager.spawn_monitor(check_interval, shutdown_rx);
        manager
    }
}
