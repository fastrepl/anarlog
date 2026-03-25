use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use tokio::sync::{Mutex, RwLock, watch};

use crate::loader::{Error, ModelLoader, ModelStatus, TryGetResult};

pub(crate) const DEFAULT_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(150);
pub(crate) const DEFAULT_CHECK_INTERVAL: Duration = Duration::from_secs(3);

pub(crate) struct ActiveModel<M> {
    pub(crate) name: String,
    pub(crate) model: Arc<M>,
}

pub(crate) enum LoadState<M: ModelLoader> {
    Loading(tokio::task::JoinHandle<Result<M, M::Error>>),
    Failed(String),
}

struct DropGuard {
    shutdown_tx: watch::Sender<()>,
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
    }
}

pub struct ModelManager<M: ModelLoader> {
    pub(crate) registry: Arc<RwLock<HashMap<String, PathBuf>>>,
    pub(crate) default_model: Arc<RwLock<Option<String>>>,
    pub(crate) active: Arc<Mutex<Option<ActiveModel<M>>>>,
    pub(crate) loading: Arc<Mutex<Option<LoadState<M>>>>,
    pub(crate) last_activity: Arc<Mutex<Option<tokio::time::Instant>>>,
    pub(crate) inactivity_timeout: Duration,
    _drop_guard: Arc<DropGuard>,
}

impl<M: ModelLoader> Clone for ModelManager<M> {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            default_model: Arc::clone(&self.default_model),
            active: Arc::clone(&self.active),
            loading: Arc::clone(&self.loading),
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

    pub async fn get_path(&self, name: &str) -> Option<PathBuf> {
        self.registry.read().await.get(name).cloned()
    }

    pub async fn get_default_path(&self) -> Option<PathBuf> {
        let default = self.default_model.read().await;
        let name = default.as_deref()?;
        self.get_path(name).await
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

    pub async fn get(&self, name: Option<&str>) -> Result<Arc<M>, Error<M::Error>> {
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

        self.update_activity().await;

        let mut active = self.active.lock().await;

        if let Some(ref a) = *active {
            if a.name == resolved {
                return Ok(Arc::clone(&a.model));
            }
        }

        *active = None;

        let model = tokio::task::spawn_blocking(move || M::load(&path))
            .await
            .map_err(|_| Error::WorkerPanicked)?
            .map_err(Error::Load)?;

        let model = Arc::new(model);
        *active = Some(ActiveModel {
            name: resolved,
            model: Arc::clone(&model),
        });

        Ok(model)
    }

    /// Non-blocking model access. Returns the cached model if available,
    /// kicks off a background load if not, and returns `Loading` while
    /// the model is being built (e.g., CoreML compilation).
    pub async fn try_get(&self, name: Option<&str>) -> TryGetResult<M> {
        let resolved: String = match name {
            Some(n) => n.to_string(),
            None => {
                let default = self.default_model.read().await;
                match default.clone() {
                    Some(n) => n,
                    None => return TryGetResult::NotRegistered,
                }
            }
        };

        let path: PathBuf = {
            let reg = self.registry.read().await;
            match reg.get(&resolved).cloned() {
                Some(p) => p,
                None => return TryGetResult::NotRegistered,
            }
        };

        // Fast path: model already loaded
        {
            let active = self.active.lock().await;
            if let Some(ref a) = *active {
                if a.name == resolved {
                    self.update_activity().await;
                    return TryGetResult::Ready(Arc::clone(&a.model));
                }
            }
        }

        // Check/start background load
        let mut loading_guard = self.loading.lock().await;

        // Check if a previous load completed or failed
        if let Some(load_state) = loading_guard.take() {
            match load_state {
                LoadState::Loading(handle) if handle.is_finished() => match handle.await {
                    Ok(Ok(model)) => {
                        let model = Arc::new(model);
                        let mut active = self.active.lock().await;
                        *active = Some(ActiveModel {
                            name: resolved,
                            model: Arc::clone(&model),
                        });
                        self.update_activity().await;
                        return TryGetResult::Ready(model);
                    }
                    Ok(Err(e)) => {
                        let msg = format!("{e}");
                        *loading_guard = Some(LoadState::Failed(msg.clone()));
                        return TryGetResult::Failed(msg);
                    }
                    Err(_) => {
                        let msg = "worker task panicked".to_string();
                        *loading_guard = Some(LoadState::Failed(msg.clone()));
                        return TryGetResult::Failed(msg);
                    }
                },
                LoadState::Loading(handle) => {
                    *loading_guard = Some(LoadState::Loading(handle));
                    return TryGetResult::Loading;
                }
                LoadState::Failed(msg) => {
                    *loading_guard = Some(LoadState::Failed(msg.clone()));
                    return TryGetResult::Failed(msg);
                }
            }
        }

        // No active load — start one
        if !path.exists() {
            return TryGetResult::NotRegistered;
        }

        self.update_activity().await;
        let handle = tokio::task::spawn_blocking(move || M::load(&path));
        *loading_guard = Some(LoadState::Loading(handle));
        TryGetResult::Loading
    }

    /// Passive model inspection. Unlike `try_get`, this does not start a load
    /// and does not refresh activity for already-ready models.
    pub async fn status(&self, name: Option<&str>) -> ModelStatus<M> {
        let resolved: String = match name {
            Some(n) => n.to_string(),
            None => {
                let default = self.default_model.read().await;
                match default.clone() {
                    Some(n) => n,
                    None => return ModelStatus::NotRegistered,
                }
            }
        };

        let path: PathBuf = {
            let reg = self.registry.read().await;
            match reg.get(&resolved).cloned() {
                Some(p) => p,
                None => return ModelStatus::NotRegistered,
            }
        };

        {
            let active = self.active.lock().await;
            if let Some(ref a) = *active {
                if a.name == resolved {
                    return ModelStatus::Ready(Arc::clone(&a.model));
                }
            }
        }

        let mut loading_guard = self.loading.lock().await;
        if let Some(load_state) = loading_guard.take() {
            match load_state {
                LoadState::Loading(handle) if handle.is_finished() => match handle.await {
                    Ok(Ok(model)) => {
                        let model = Arc::new(model);
                        let mut active = self.active.lock().await;
                        *active = Some(ActiveModel {
                            name: resolved,
                            model: Arc::clone(&model),
                        });
                        return ModelStatus::Ready(model);
                    }
                    Ok(Err(e)) => {
                        let msg = format!("{e}");
                        *loading_guard = Some(LoadState::Failed(msg.clone()));
                        return ModelStatus::Failed(msg);
                    }
                    Err(_) => {
                        let msg = "worker task panicked".to_string();
                        *loading_guard = Some(LoadState::Failed(msg.clone()));
                        return ModelStatus::Failed(msg);
                    }
                },
                LoadState::Loading(handle) => {
                    *loading_guard = Some(LoadState::Loading(handle));
                    return ModelStatus::Loading;
                }
                LoadState::Failed(msg) => {
                    *loading_guard = Some(LoadState::Failed(msg.clone()));
                    return ModelStatus::Failed(msg);
                }
            }
        }

        if !path.exists() {
            return ModelStatus::NotRegistered;
        }

        ModelStatus::Idle
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
        let loading = Arc::clone(&self.loading);
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
                        if let Some(t) = *last {
                            if t.elapsed() > inactivity_timeout {
                                *active.lock().await = None;
                                *loading.lock().await = None;
                            }
                        }
                    }
                }
            }
        });
    }

    pub(crate) fn new(
        registry: Arc<RwLock<HashMap<String, PathBuf>>>,
        default_model: Arc<RwLock<Option<String>>>,
        inactivity_timeout: Duration,
        shutdown_tx: watch::Sender<()>,
    ) -> Self {
        Self {
            registry,
            default_model,
            active: Arc::new(Mutex::new(None)),
            loading: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(Mutex::new(None)),
            inactivity_timeout,
            _drop_guard: Arc::new(DropGuard { shutdown_tx }),
        }
    }
}
