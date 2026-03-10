use std::path::PathBuf;

use tauri::Manager;
use tauri_plugin_store2::Store2PluginExt;
use tauri_plugin_updater::UpdaterExt;
use tauri_specta::Event;

use crate::events::{
    UpdateDownloadFailedEvent, UpdateDownloadingEvent, UpdateReadyEvent, UpdatedEvent,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallResult {
    RelaunchCurrent,
    MacosBundleUpdate {
        current_path: String,
        staged_path: String,
        target_path: String,
        backup_path: String,
        stage_dir: String,
    },
}

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

    pub async fn install(&self, version: &str) -> Result<InstallResult, crate::Error> {
        let bytes = self.get_cached_update_bytes(version)?;

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

        if let Ok(store) = self.manager.store2().store() {
            let _ = store.save();
        }

        #[cfg(target_os = "macos")]
        {
            let stage_dir = self.create_macos_update_stage_dir(version)?;
            let staged_update = match crate::migration::stage_macos_update(&bytes, &stage_dir) {
                Ok(staged_update) => staged_update,
                Err(err) => {
                    let _ = std::fs::remove_dir_all(&stage_dir);
                    return Err(err);
                }
            };

            Ok(InstallResult::MacosBundleUpdate {
                current_path: staged_update.current_app_path.display().to_string(),
                staged_path: staged_update.staged_app_path.display().to_string(),
                target_path: staged_update.target_app_path.display().to_string(),
                backup_path: staged_update.current_backup_path.display().to_string(),
                stage_dir: staged_update.stage_dir.display().to_string(),
            })
        }

        #[cfg(not(target_os = "macos"))]
        {
            update.install(&bytes)?;
            Ok(InstallResult::RelaunchCurrent)
        }
    }

    pub async fn postinstall(&self, result: InstallResult) -> Result<(), crate::Error> {
        match result {
            InstallResult::RelaunchCurrent => {
                self.manager.app_handle().restart();
            }
            InstallResult::MacosBundleUpdate {
                current_path,
                staged_path,
                target_path,
                backup_path,
                stage_dir,
            } => {
                #[cfg(target_os = "macos")]
                {
                    let handle = self.manager.app_handle().clone();
                    let current_pid = std::process::id();

                    crate::migration::schedule_macos_update_after_exit(
                        current_pid,
                        crate::migration::StagedMacosUpdate {
                            current_app_path: PathBuf::from(current_path),
                            staged_app_path: PathBuf::from(staged_path),
                            target_app_path: PathBuf::from(target_path),
                            current_backup_path: PathBuf::from(backup_path),
                            stage_dir: PathBuf::from(stage_dir),
                        },
                    )?;

                    handle.exit(0);
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = (
                        current_path,
                        staged_path,
                        target_path,
                        backup_path,
                        stage_dir,
                    );
                    return Err(crate::Error::InvalidPostinstallState(
                        "macos_bundle_update is only valid on macOS".into(),
                    ));
                }
            }
        }
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

#[cfg(target_os = "macos")]
impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Updater2<'a, R, M> {
    fn create_macos_update_stage_dir(&self, version: &str) -> Result<PathBuf, crate::Error> {
        let base_dir = self
            .manager
            .app_handle()
            .path()
            .app_cache_dir()
            .map_err(|_| crate::Error::CachePathUnavailable)?
            .join("updates")
            .join("staged");
        std::fs::create_dir_all(&base_dir)?;

        let version = sanitize_path_component(version);
        let current_pid = std::process::id();

        for attempt in 0..100 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let stage_dir = base_dir.join(format!("{version}-{current_pid}-{nanos}-{attempt}"));

            match std::fs::create_dir(&stage_dir) {
                Ok(()) => return Ok(stage_dir),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(err.into()),
            }
        }

        Err(crate::Error::CachePathUnavailable)
    }
}

#[cfg(target_os = "macos")]
fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '_',
        })
        .collect()
}
