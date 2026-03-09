use std::path::PathBuf;

use tauri::Manager;
use tauri_plugin_store2::Store2PluginExt;
use tauri_plugin_updater::UpdaterExt;
use tauri_specta::Event;

use crate::events::{
    UpdateDownloadFailedEvent, UpdateDownloadingEvent, UpdateReadyEvent, UpdatedEvent,
};

pub struct Updater2<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Updater2<'a, R, M> {
    pub fn get_last_seen_version(&self) -> Result<Option<String>, crate::Error> {
        let store = self.manager.store2().scoped_store(crate::PLUGIN_NAME)?;
        let v = store.get(crate::StoreKey::LastSeenVersion)?;
        Ok(v)
    }

    pub fn set_last_seen_version(&self, version: String) -> Result<(), crate::Error> {
        let store = self.manager.store2().scoped_store(crate::PLUGIN_NAME)?;
        store.set(crate::StoreKey::LastSeenVersion, version)?;
        Ok(())
    }

    pub fn maybe_emit_updated(&self) {
        let current_version = match self.manager.config().version.as_ref() {
            Some(v) => v.clone(),
            None => {
                tracing::warn!("no_version_in_config");
                return;
            }
        };

        let (should_emit, previous) = match self.get_last_seen_version() {
            Ok(Some(last_version)) if !last_version.is_empty() => {
                (last_version != current_version, Some(last_version))
            }
            Ok(_) => (false, None),
            Err(e) => {
                tracing::error!("failed_to_get_last_seen_version: {}", e);
                (false, None)
            }
        };

        if should_emit {
            let payload = UpdatedEvent {
                previous,
                current: current_version.clone(),
            };

            if let Err(e) = payload.emit(self.manager.app_handle()) {
                tracing::error!("failed_to_emit_updated_event: {}", e);
            }
        }

        if let Err(e) = self.set_last_seen_version(current_version) {
            tracing::error!("failed_to_update_version: {}", e);
        }
    }

    fn cache_update_bytes(&self, version: &str, bytes: &[u8]) -> Result<(), crate::Error> {
        let cache_path =
            get_cache_path(self.manager, version).ok_or(crate::Error::CachePathUnavailable)?;

        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&cache_path, bytes)?;
        tracing::debug!("cached_update_bytes: {:?}", cache_path);
        Ok(())
    }

    fn get_cached_update_bytes(&self, version: &str) -> Result<Vec<u8>, crate::Error> {
        let cache_path =
            get_cache_path(self.manager, version).ok_or(crate::Error::CachePathUnavailable)?;

        if !cache_path.exists() {
            return Err(crate::Error::CachedUpdateNotFound);
        }

        let bytes = std::fs::read(&cache_path)?;
        Ok(bytes)
    }

    pub async fn check(&self) -> Result<Option<String>, crate::Error> {
        let updater = self.manager.updater()?;
        let update = updater.check().await?;
        Ok(update.map(|u| u.version))
    }

    fn has_cached_update(&self, version: &str) -> bool {
        get_cache_path(self.manager, version)
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    pub async fn download(&self, version: &str) -> Result<(), crate::Error> {
        if self.has_cached_update(version) {
            let _ = UpdateReadyEvent {
                version: version.to_string(),
            }
            .emit(self.manager.app_handle());
            return Ok(());
        }

        use tauri_plugin_fs_db::FsDbPluginExt;
        if let Err(e) = self.manager.fs_db().ensure_version_file() {
            tracing::warn!("failed_to_ensure_version_file: {}", e);
        }

        let updater = self.manager.updater()?;
        let update = updater
            .check()
            .await?
            .ok_or(crate::Error::UpdateNotAvailable)?;

        if update.version != version {
            return Err(crate::Error::VersionMismatch {
                expected: version.to_string(),
                actual: update.version,
            });
        }

        let _ = UpdateDownloadingEvent {
            version: version.to_string(),
        }
        .emit(self.manager.app_handle());

        let result: Result<(), crate::Error> = async {
            let bytes = update.download(|_, _| {}, || {}).await?;
            self.cache_update_bytes(version, &bytes)?;
            Ok(())
        }
        .await;

        if let Err(e) = &result {
            tracing::error!("download_failed: {}", e);
            let _ = UpdateDownloadFailedEvent {
                version: version.to_string(),
            }
            .emit(self.manager.app_handle());
            return Err(result.unwrap_err());
        }

        let _ = UpdateReadyEvent {
            version: version.to_string(),
        }
        .emit(self.manager.app_handle());

        Ok(())
    }

    pub async fn install(&self, version: &str) -> Result<(), crate::Error> {
        let bytes = self.get_cached_update_bytes(version)?;

        let updater = self.manager.updater()?;
        let update = updater
            .check()
            .await?
            .ok_or(crate::Error::UpdateNotAvailable)?;

        if let Ok(store) = self.manager.store2().store() {
            let _ = store.save();
        }

        update.install(&bytes)?;

        Ok(())
    }
}

pub trait Updater2PluginExt<R: tauri::Runtime> {
    fn updater2(&self) -> Updater2<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> Updater2PluginExt<R> for T {
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

fn get_cache_path<R: tauri::Runtime, M: tauri::Manager<R>>(
    manager: &M,
    version: &str,
) -> Option<PathBuf> {
    let dir = manager
        .app_handle()
        .path()
        .app_cache_dir()
        .ok()
        .map(|p: PathBuf| p.join("updates"))?;
    Some(dir.join(format!("{}.bin", version)))
}
