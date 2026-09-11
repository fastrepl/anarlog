use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use anlg_desktop_updater::{UpdateBackend, UpdateEvents, UpdatePolicy, Updater};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store2::Store2PluginExt;
use tauri_plugin_updater::UpdaterExt;
use tauri_specta::Event;

use crate::events::{
    UpdateAvailableEvent, UpdateDownloadFailedEvent, UpdateDownloadProgressEvent,
    UpdateDownloadingEvent, UpdateReadyEvent, UpdatedEvent,
};

static MEETING_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct TauriUpdateBackend<R: Runtime> {
    app: AppHandle<R>,
    update: Mutex<Option<tauri_plugin_updater::Update>>,
}

impl<R: Runtime> TauriUpdateBackend<R> {
    fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            update: Mutex::new(None),
        }
    }
}

impl<R: Runtime> UpdateBackend for TauriUpdateBackend<R> {
    fn check(
        &self,
    ) -> Pin<Box<dyn Future<Output = anlg_desktop_updater::Result<Option<String>>> + Send + '_>>
    {
        Box::pin(async move {
            let update = self
                .app
                .updater()
                .map_err(|error| anlg_desktop_updater::Error::Backend(error.to_string()))?
                .check()
                .await
                .map_err(|error| anlg_desktop_updater::Error::Backend(error.to_string()))?;
            let version = update.as_ref().map(|update| update.version.clone());
            *self.update.lock().unwrap() = update;
            Ok(version)
        })
    }

    fn download<'a>(
        &'a self,
        version: &'a str,
        on_progress: &'a (dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Pin<Box<dyn Future<Output = anlg_desktop_updater::Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            let update = self
                .app
                .updater()
                .map_err(|error| anlg_desktop_updater::Error::Backend(error.to_string()))?
                .check()
                .await
                .map_err(|error| anlg_desktop_updater::Error::Backend(error.to_string()))?
                .ok_or(anlg_desktop_updater::Error::UpdateNotAvailable)?;
            if update.version != version {
                return Err(anlg_desktop_updater::Error::VersionMismatch {
                    expected: version.to_string(),
                    actual: update.version,
                });
            }
            update
                .download(|chunk, total| on_progress(chunk as u64, total), || {})
                .await
                .map_err(|error| anlg_desktop_updater::Error::Backend(error.to_string()))
        })
    }

    fn install(&self, version: &str, bytes: &[u8]) -> anlg_desktop_updater::Result<()> {
        let mut update = self
            .update
            .lock()
            .unwrap()
            .take()
            .ok_or(anlg_desktop_updater::Error::UpdateNotAvailable)?;
        update.version = version.to_string();
        if let Err(error) = self.app.store2().save() {
            tracing::warn!(%error, "failed_to_persist_update_store");
        }
        update
            .install(bytes)
            .map_err(|error| anlg_desktop_updater::Error::Backend(error.to_string()))?;
        self.app.restart();
    }
}

pub(crate) struct SharedUpdater<R: Runtime>(
    Arc<Updater<TauriUpdateBackend<R>, TauriUpdateEvents<R>>>,
);

struct TauriUpdateEvents<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> UpdateEvents for TauriUpdateEvents<R> {
    fn available(&self, version: &str) {
        let _ = (UpdateAvailableEvent {
            version: version.to_string(),
        })
        .emit(&self.app);
    }

    fn downloading(&self, version: &str) {
        let _ = (UpdateDownloadingEvent {
            version: version.to_string(),
        })
        .emit(&self.app);
    }

    fn progress(&self, version: &str, chunk: u64, total: Option<u64>) {
        let _ = (UpdateDownloadProgressEvent {
            version: version.to_string(),
            chunk_length: chunk,
            content_length: total,
        })
        .emit(&self.app);
    }

    fn download_failed(&self, version: &str) {
        let _ = (UpdateDownloadFailedEvent {
            version: version.to_string(),
        })
        .emit(&self.app);
    }

    fn ready(&self, version: &str) {
        let _ = (UpdateReadyEvent {
            version: version.to_string(),
        })
        .emit(&self.app);
    }
}

pub struct Updater2<'a, R: Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime, M: tauri::Manager<R>> Updater2<'_, R, M> {
    pub fn automatic_updates_enabled(&self) -> Result<bool, crate::Error> {
        let store = self.manager.store2().scoped_store(crate::PLUGIN_NAME)?;
        Ok(store
            .get(crate::StoreKey::AutomaticUpdatesEnabled)?
            .unwrap_or(true))
    }

    pub fn set_automatic_updates_enabled(&self, enabled: bool) -> Result<(), crate::Error> {
        let store = self.manager.store2().scoped_store(crate::PLUGIN_NAME)?;
        store.set(crate::StoreKey::AutomaticUpdatesEnabled, enabled)?;
        Ok(())
    }

    pub fn meeting_active(&self) -> bool {
        MEETING_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_meeting_active(&self, active: bool) {
        MEETING_ACTIVE.store(active, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_last_seen_version(&self) -> Result<Option<String>, crate::Error> {
        let store = self.manager.store2().scoped_store(crate::PLUGIN_NAME)?;
        Ok(store.get(crate::StoreKey::LastSeenVersion)?)
    }

    pub fn set_last_seen_version(&self, version: String) -> Result<(), crate::Error> {
        let store = self.manager.store2().scoped_store(crate::PLUGIN_NAME)?;
        store.set(crate::StoreKey::LastSeenVersion, version)?;
        Ok(())
    }

    pub fn maybe_emit_updated(&self) {
        let Some(current_version) = self.manager.config().version.clone() else {
            tracing::warn!("no_version_in_config");
            return;
        };
        let (should_emit, previous) = match self.get_last_seen_version() {
            Ok(Some(last_version)) if !last_version.is_empty() => {
                (last_version != current_version, Some(last_version))
            }
            Ok(_) => (false, None),
            Err(error) => {
                tracing::error!("failed_to_get_last_seen_version: {}", error);
                (false, None)
            }
        };
        if should_emit {
            if let Err(error) = (UpdatedEvent {
                previous,
                current: current_version.clone(),
            })
            .emit(self.manager.app_handle())
            {
                tracing::error!("failed_to_emit_updated_event: {}", error);
            }
        }
        if let Err(error) = self.set_last_seen_version(current_version) {
            tracing::error!("failed_to_update_version: {}", error);
        }
    }

    fn core(
        &self,
    ) -> Result<Arc<Updater<TauriUpdateBackend<R>, TauriUpdateEvents<R>>>, crate::Error> {
        self.manager
            .app_handle()
            .try_state::<SharedUpdater<R>>()
            .map(|state| state.0.clone())
            .ok_or(crate::Error::UpdaterNotManaged)
    }

    pub async fn check(&self) -> Result<Option<String>, crate::Error> {
        Ok(self.core()?.check().await?)
    }

    pub fn has_cached_update(&self, version: &str) -> bool {
        self.core()
            .map(|updater| updater.has_cached_update(version))
            .unwrap_or(false)
    }

    pub async fn download(&self, version: &str) -> Result<(), crate::Error> {
        Ok(self.core()?.download(version).await?)
    }

    pub async fn install_and_relaunch(&self, version: &str) -> Result<(), crate::Error> {
        Ok(self.core()?.install_and_relaunch(version).await?)
    }

    pub async fn tick(&self, install_at_open: bool) -> bool {
        let updater = match self.core() {
            Ok(updater) => updater,
            Err(error) => {
                tracing::error!("updater_initialization_failed: {}", error);
                return false;
            }
        };
        let app = self.manager.app_handle().clone();
        let policy = move || UpdatePolicy {
            automatic_updates_enabled: app.updater2().automatic_updates_enabled().unwrap_or_else(
                |error| {
                    tracing::error!("automatic_update_policy_read_failed: {}", error);
                    false
                },
            ),
            meeting_active: MEETING_ACTIVE.load(std::sync::atomic::Ordering::Relaxed),
        };
        updater.tick(&policy, install_at_open).await
    }
}

pub(crate) fn create_core<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<SharedUpdater<R>, crate::Error> {
    let updates_dir = app
        .path()
        .app_cache_dir()
        .map_err(|_| crate::Error::CachePathUnavailable)?
        .join("updates");
    let current_version = app.config().version.clone().unwrap_or_default();
    Ok(SharedUpdater(Arc::new(Updater::new(
        Arc::new(TauriUpdateBackend::new(app.clone())),
        Arc::new(TauriUpdateEvents { app: app.clone() }),
        updates_dir,
        current_version,
    ))))
}

pub trait Updater2PluginExt<R: Runtime> {
    fn updater2(&self) -> Updater2<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: Runtime, T: tauri::Manager<R>> Updater2PluginExt<R> for T {
    fn updater2(&self) -> Updater2<'_, R, Self>
    where
        Self: Sized,
    {
        Updater2 {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
