use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use tokio::sync::{Mutex, RwLock, watch};

use crate::builder::DropGuard;
use crate::{Error, ModelLoader};

pub(crate) struct ActiveModel<M> {
    pub(crate) name: String,
    pub(crate) model: Arc<M>,
}

pub struct ModelManager<M: ModelLoader> {
    pub(crate) registry: Arc<RwLock<HashMap<String, PathBuf>>>,
    pub(crate) default_model: Arc<RwLock<Option<String>>>,
    pub(crate) active: Arc<Mutex<Option<ActiveModel<M>>>>,
    pub(crate) last_activity: Arc<Mutex<Option<tokio::time::Instant>>>,
    pub(crate) inactivity_timeout: Duration,
    pub(crate) _drop_guard: Arc<DropGuard>,
}

impl<M: ModelLoader> Clone for ModelManager<M> {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            default_model: Arc::clone(&self.default_model),
            active: Arc::clone(&self.active),
            last_activity: Arc::clone(&self.last_activity),
            inactivity_timeout: self.inactivity_timeout,
            _drop_guard: Arc::clone(&self._drop_guard),
        }
    }
}

impl<M: ModelLoader> ModelManager<M> {
    pub fn builder() -> crate::ModelManagerBuilder<M> {
        crate::ModelManagerBuilder::default()
    }

    pub fn default_model_name(&self) -> Option<String> {
        self.default_model
            .try_read()
            .map(|default_model| default_model.clone())
            .expect("default model lock should not be contended during service setup")
    }

    pub fn has_default_model(&self) -> bool {
        self.default_model
            .try_read()
            .map(|default_model| default_model.is_some())
            .expect("default model lock should not be contended during service setup")
    }

    pub async fn register(&self, name: impl Into<String>, path: impl Into<PathBuf>) {
        let mut reg = self.registry.write().await;
        reg.insert(name.into(), path.into());
    }

    pub async fn unregister(&self, name: &str) {
        let mut reg = self.registry.write().await;
        reg.remove(name);

        let mut active = self.active.lock().await;
        if active.as_ref().is_some_and(|a| a.name == name) {
            *active = None;
        }
    }

    pub async fn set_default(&self, name: impl Into<String>) {
        let mut default = self.default_model.write().await;
        *default = Some(name.into());
    }

    pub async fn get(&self, name: Option<&str>) -> Result<Arc<M>, Error> {
        let resolved = match name {
            Some(n) => n.to_string(),
            None => {
                let default = self.default_model.read().await;
                default.clone().ok_or(Error::NoDefaultModel)?
            }
        };

        let path = {
            let reg = self.registry.read().await;
            reg.get(&resolved)
                .cloned()
                .ok_or_else(|| Error::ModelNotRegistered(resolved.clone()))?
        };

        if !path.exists() {
            return Err(Error::ModelFileNotFound(path.display().to_string()));
        }

        let mut active = self.active.lock().await;
        let mut last_activity = self.last_activity.lock().await;
        let now = tokio::time::Instant::now();

        if last_activity.is_some_and(|t| now.duration_since(t) > self.inactivity_timeout) {
            *active = None;
        }
        *last_activity = Some(now);

        if let Some(ref a) = *active
            && a.name == resolved
        {
            return Ok(Arc::clone(&a.model));
        }

        *active = None;

        let model = tokio::task::spawn_blocking(move || M::load(&path))
            .await
            .map_err(|_| Error::WorkerPanicked)?
            .map_err(|e| Error::Load(Box::new(e)))?;

        let model = Arc::new(model);
        *active = Some(ActiveModel {
            name: resolved,
            model: Arc::clone(&model),
        });

        Ok(model)
    }

    pub async fn keep_alive(&self) {
        self.update_activity().await;
    }

    async fn update_activity(&self) {
        *self.last_activity.lock().await = Some(tokio::time::Instant::now());
    }

    pub(crate) fn spawn_monitor(
        &self,
        check_interval: Duration,
        mut shutdown_rx: watch::Receiver<()>,
    ) {
        let active = Arc::clone(&self.active);
        let last_activity = Arc::clone(&self.last_activity);
        let inactivity_timeout = self.inactivity_timeout;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => break,
                    _ = interval.tick() => {
                        let last = last_activity.lock().await;
                        if let Some(t) = *last
                            && t.elapsed() > inactivity_timeout
                        {
                            *active.lock().await = None;
                        }
                    }
                }
            }
        });
    }
}
