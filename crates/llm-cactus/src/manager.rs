use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tokio::sync::{Mutex, RwLock, watch};

const DEFAULT_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(150);
const DEFAULT_CHECK_INTERVAL: Duration = Duration::from_secs(3);

pub trait ModelLoader: Send + Sync + 'static {
    fn load(path: &Path) -> Result<Self, crate::Error>
    where
        Self: Sized;
}

impl ModelLoader for hypr_cactus::Model {
    fn load(path: &Path) -> Result<Self, crate::Error> {
        Ok(hypr_cactus::Model::new(path)?)
    }
}

struct ActiveModel<M> {
    name: String,
    model: Arc<M>,
}

struct DropGuard {
    shutdown_tx: watch::Sender<()>,
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
    }
}

pub struct ModelManager<M: ModelLoader = hypr_cactus::Model> {
    registry: Arc<RwLock<HashMap<String, PathBuf>>>,
    default_model: Arc<RwLock<Option<String>>>,
    active: Arc<Mutex<Option<ActiveModel<M>>>>,
    last_activity: Arc<Mutex<Option<tokio::time::Instant>>>,
    inactivity_timeout: Duration,
    _drop_guard: Arc<DropGuard>,
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
    pub fn builder() -> ModelManagerBuilder<M> {
        ModelManagerBuilder::default()
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

    pub async fn get(&self, name: Option<&str>) -> Result<Arc<M>, crate::Error> {
        let resolved = match name {
            Some(n) => n.to_string(),
            None => {
                let default = self.default_model.read().await;
                default.clone().ok_or(crate::Error::NoDefaultModel)?
            }
        };

        let path = {
            let reg = self.registry.read().await;
            reg.get(&resolved)
                .cloned()
                .ok_or_else(|| crate::Error::ModelNotRegistered(resolved.clone()))?
        };

        if !path.exists() {
            return Err(crate::Error::ModelFileNotFound(path.display().to_string()));
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
            .map_err(|_| crate::Error::WorkerPanicked)??;

        let model = Arc::new(model);
        *active = Some(ActiveModel {
            name: resolved,
            model: Arc::clone(&model),
        });

        Ok(model)
    }

    async fn update_activity(&self) {
        *self.last_activity.lock().await = Some(tokio::time::Instant::now());
    }

    fn spawn_monitor(&self, check_interval: Duration, mut shutdown_rx: watch::Receiver<()>) {
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
                        if let Some(t) = *last {
                            if t.elapsed() > inactivity_timeout {
                                *active.lock().await = None;
                            }
                        }
                    }
                }
            }
        });
    }
}

pub struct ModelManagerBuilder<M: ModelLoader = hypr_cactus::Model> {
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

#[cfg(test)]
mod tests {
    use super::*;

    struct MockModel;

    impl ModelLoader for MockModel {
        fn load(_path: &Path) -> Result<Self, crate::Error> {
            Ok(MockModel)
        }
    }

    fn temp_model_path() -> PathBuf {
        let dir = std::env::temp_dir().join("llm-cactus-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.bin", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"").unwrap();
        path
    }

    fn build_manager(
        timeout: Duration,
        check_interval: Duration,
        models: &[(&str, PathBuf)],
    ) -> ModelManager<MockModel> {
        let mut builder = ModelManager::<MockModel>::builder()
            .inactivity_timeout(timeout)
            .check_interval(check_interval);
        for (name, path) in models {
            builder = builder.register(*name, path.clone());
        }
        builder.build()
    }

    #[tokio::test(start_paused = true)]
    async fn idle_model_gets_evicted() {
        let path = temp_model_path();
        let mgr = build_manager(
            Duration::from_millis(100),
            Duration::from_millis(10),
            &[("a", path)],
        );

        let m1 = mgr.get(Some("a")).await.unwrap();
        let m2 = mgr.get(Some("a")).await.unwrap();
        assert!(Arc::ptr_eq(&m1, &m2));

        tokio::time::advance(Duration::from_millis(120)).await;
        tokio::task::yield_now().await;

        let m3 = mgr.get(Some("a")).await.unwrap();
        assert!(!Arc::ptr_eq(&m1, &m3));
    }

    #[tokio::test(start_paused = true)]
    async fn activity_prevents_eviction() {
        let path = temp_model_path();
        let mgr = build_manager(
            Duration::from_millis(100),
            Duration::from_millis(10),
            &[("a", path)],
        );

        let m1 = mgr.get(Some("a")).await.unwrap();

        for _ in 0..5 {
            tokio::time::advance(Duration::from_millis(50)).await;
            tokio::task::yield_now().await;

            let m = mgr.get(Some("a")).await.unwrap();
            assert!(Arc::ptr_eq(&m1, &m));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn access_near_timeout_resets_timer() {
        let path = temp_model_path();
        let mgr = build_manager(
            Duration::from_millis(100),
            Duration::from_millis(10),
            &[("a", path)],
        );

        let m1 = mgr.get(Some("a")).await.unwrap();

        tokio::time::advance(Duration::from_millis(90)).await;
        tokio::task::yield_now().await;

        let m2 = mgr.get(Some("a")).await.unwrap();
        assert!(Arc::ptr_eq(&m1, &m2));

        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;

        let m3 = mgr.get(Some("a")).await.unwrap();
        assert!(Arc::ptr_eq(&m1, &m3));
    }
}
