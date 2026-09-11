use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("update not available")]
    UpdateNotAvailable,
    #[error("version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
    #[error("cached update {version} is not newer than current {current}")]
    UpdateNotNewer { version: String, current: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Backend(String),
}

pub trait UpdateBackend: Send + Sync {
    fn check(&self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>>;

    fn download<'a>(
        &'a self,
        version: &'a str,
        on_progress: &'a (dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;

    fn install(&self, version: &str, bytes: &[u8]) -> Result<()>;

    fn supports_install(&self) -> bool {
        true
    }
}

pub trait UpdateEvents: Send + Sync {
    fn available(&self, version: &str);
    fn downloading(&self, version: &str);
    fn progress(&self, version: &str, chunk: u64, total: Option<u64>);
    fn download_failed(&self, version: &str);
    fn ready(&self, version: &str);
}

#[derive(Debug, Clone, Copy)]
pub struct UpdatePolicy {
    pub automatic_updates_enabled: bool,
    pub meeting_active: bool,
}

pub trait UpdatePolicySource: Send + Sync {
    fn automatic_updates_enabled(&self) -> bool;
    fn meeting_active(&self) -> bool;
}

impl<F: Fn() -> UpdatePolicy + Send + Sync> UpdatePolicySource for F {
    fn automatic_updates_enabled(&self) -> bool {
        self().automatic_updates_enabled
    }

    fn meeting_active(&self) -> bool {
        self().meeting_active
    }
}

pub struct Updater<B: UpdateBackend, E: UpdateEvents> {
    backend: Arc<B>,
    events: Arc<E>,
    updates_dir: PathBuf,
    current_version: String,
    download_mutex: tokio::sync::Mutex<()>,
}

impl<B: UpdateBackend, E: UpdateEvents> Updater<B, E> {
    pub fn new(
        backend: Arc<B>,
        events: Arc<E>,
        updates_dir: impl Into<PathBuf>,
        current_version: impl Into<String>,
    ) -> Self {
        Self {
            backend,
            events,
            updates_dir: updates_dir.into(),
            current_version: current_version.into(),
            download_mutex: tokio::sync::Mutex::new(()),
        }
    }

    pub async fn check(&self) -> Result<Option<String>> {
        let version = self.backend.check().await?;
        prune_updates_dir(&self.updates_dir, version.as_deref());
        if let Some(version) = &version {
            if self.has_cached_update(version) {
                self.events.ready(version);
            } else {
                self.events.available(version);
            }
        }
        Ok(version)
    }

    pub fn has_cached_update(&self, version: &str) -> bool {
        cache_path(&self.updates_dir, version).is_file()
    }

    pub async fn download(&self, version: &str) -> Result<()> {
        let _guard = self.download_mutex.lock().await;
        if self.has_cached_update(version) {
            self.events.ready(version);
            return Ok(());
        }

        let actual = self
            .backend
            .check()
            .await?
            .ok_or(Error::UpdateNotAvailable)?;
        if actual != version {
            return Err(Error::VersionMismatch {
                expected: version.to_string(),
                actual,
            });
        }

        self.events.downloading(version);
        let progress_version = version.to_string();
        let events = self.events.clone();
        let on_progress = move |chunk, total| events.progress(&progress_version, chunk, total);
        let result = self.backend.download(version, &on_progress).await;
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(error) => {
                self.events.download_failed(version);
                return Err(error);
            }
        };

        if let Err(error) = cache_update_bytes(&self.updates_dir, version, &bytes) {
            self.events.download_failed(version);
            return Err(error);
        }
        self.events.ready(version);
        Ok(())
    }

    pub async fn install_and_relaunch(&self, version: &str) -> Result<()> {
        let current = self.current_version.clone();
        let is_newer = match (
            semver::Version::parse(version),
            semver::Version::parse(&current),
        ) {
            (Ok(cached), Ok(current)) => cached > current,
            _ => false,
        };
        if !is_newer {
            return Err(Error::UpdateNotNewer {
                version: version.to_string(),
                current,
            });
        }

        let bytes = get_cached_update_bytes(&self.updates_dir, version)?;
        self.backend
            .check()
            .await?
            .ok_or(Error::UpdateNotAvailable)?;
        self.backend.install(version, &bytes)
    }

    pub async fn tick(&self, policy: &dyn UpdatePolicySource, install_at_open: bool) -> bool {
        if !policy.automatic_updates_enabled() {
            return false;
        }
        if policy.meeting_active() {
            return install_at_open;
        }

        let Some(version) = (match self.check().await {
            Ok(version) => version,
            Err(error) => {
                tracing::error!(%error, "update_check_failed");
                return install_at_open;
            }
        }) else {
            return false;
        };

        if policy.meeting_active() {
            return install_at_open;
        }

        if install_at_open && self.has_cached_update(&version) {
            if policy.meeting_active() {
                return true;
            }
            if !self.backend.supports_install() {
                tracing::debug!("update_install_unsupported");
                return false;
            }
            return match self.install_and_relaunch(&version).await {
                Ok(()) => false,
                Err(error) => {
                    tracing::error!(%error, "cached_update_install_failed");
                    true
                }
            };
        }

        if let Err(error) = self.download(&version).await {
            tracing::error!(%error, "update_download_failed");
            return install_at_open;
        }

        if install_at_open {
            if policy.meeting_active() {
                return true;
            }
            if !self.backend.supports_install() {
                tracing::debug!("update_install_unsupported");
                return false;
            }
            if let Err(error) = self.install_and_relaunch(&version).await {
                tracing::error!(%error, "downloaded_update_install_failed");
                return true;
            }
        }
        false
    }
}

pub fn cache_path(updates_dir: &Path, version: &str) -> PathBuf {
    updates_dir.join(format!("{version}.bin"))
}

pub fn cache_update_bytes(updates_dir: &Path, version: &str, bytes: &[u8]) -> Result<()> {
    let path = cache_path(updates_dir, version);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

pub fn get_cached_update_bytes(updates_dir: &Path, version: &str) -> Result<Vec<u8>> {
    Ok(std::fs::read(cache_path(updates_dir, version))?)
}

pub fn prune_updates_dir(dir: &Path, keep: Option<&str>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("bin") {
            continue;
        }
        if keep.is_some() && path.file_stem().and_then(|stem| stem.to_str()) == keep {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::info!(?path, "pruned_cached_update"),
            Err(error) => tracing::warn!(?path, %error, "failed_to_prune_cached_update"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    #[derive(Default)]
    struct Backend {
        check: Mutex<Option<Result<Option<String>>>>,
        installs: Mutex<Vec<String>>,
    }

    impl UpdateBackend for Backend {
        fn check(&self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
            Box::pin(async {
                self.check
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap_or(Ok(Some("2.0.0".to_string())))
            })
        }

        fn download<'a>(
            &'a self,
            _version: &'a str,
            _on_progress: &'a (dyn Fn(u64, Option<u64>) + Send + Sync),
        ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
            Box::pin(async { Ok(b"update".to_vec()) })
        }

        fn install(&self, version: &str, _bytes: &[u8]) -> Result<()> {
            self.installs.lock().unwrap().push(version.to_string());
            Ok(())
        }
    }

    #[derive(Default)]
    struct Events;

    impl UpdateEvents for Events {
        fn available(&self, _version: &str) {}
        fn downloading(&self, _version: &str) {}
        fn progress(&self, _version: &str, _chunk: u64, _total: Option<u64>) {}
        fn download_failed(&self, _version: &str) {}
        fn ready(&self, _version: &str) {}
    }

    fn updater(backend: Arc<Backend>, dir: &Path) -> Updater<Backend, Events> {
        Updater::new(backend, Arc::new(Events), dir, "1.0.0")
    }

    #[tokio::test]
    async fn disabled_policy_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(Backend::default());
        assert!(
            !updater(backend, dir.path())
                .tick(
                    &|| UpdatePolicy {
                        automatic_updates_enabled: false,
                        meeting_active: false,
                    },
                    true
                )
                .await
        );
    }

    #[tokio::test]
    async fn meeting_preserves_install_intent() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(Backend::default());
        assert!(
            updater(backend, dir.path())
                .tick(
                    &|| UpdatePolicy {
                        automatic_updates_enabled: true,
                        meeting_active: true,
                    },
                    true
                )
                .await
        );
        assert!(
            !updater(Arc::new(Backend::default()), dir.path())
                .tick(
                    &|| UpdatePolicy {
                        automatic_updates_enabled: true,
                        meeting_active: true,
                    },
                    false
                )
                .await
        );
    }

    #[tokio::test]
    async fn cached_update_installs_at_open() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(Backend::default());
        cache_update_bytes(dir.path(), "2.0.0", b"update").unwrap();
        assert!(
            !updater(backend.clone(), dir.path())
                .tick(
                    &|| UpdatePolicy {
                        automatic_updates_enabled: true,
                        meeting_active: false,
                    },
                    true
                )
                .await
        );
        assert_eq!(&*backend.installs.lock().unwrap(), &["2.0.0"]);
    }

    #[tokio::test]
    async fn download_failure_preserves_install_intent() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(FailingDownloadBackend);
        assert!(
            updater_with_backend(backend, dir.path())
                .tick(
                    &|| UpdatePolicy {
                        automatic_updates_enabled: true,
                        meeting_active: false,
                    },
                    true
                )
                .await
        );
    }

    #[tokio::test]
    async fn check_failure_preserves_install_intent() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(FailingCheckBackend);
        assert!(
            updater_with_backend(backend, dir.path())
                .tick(
                    &|| UpdatePolicy {
                        automatic_updates_enabled: true,
                        meeting_active: false,
                    },
                    true
                )
                .await
        );
    }

    #[tokio::test]
    async fn rejects_non_newer_cached_updates() {
        let dir = tempfile::tempdir().unwrap();
        cache_update_bytes(dir.path(), "1.0.0", b"update").unwrap();
        let result = updater(Arc::new(Backend::default()), dir.path())
            .install_and_relaunch("1.0.0")
            .await;
        assert!(matches!(result, Err(Error::UpdateNotNewer { .. })));
    }

    fn updater_with_backend<B: UpdateBackend>(backend: Arc<B>, dir: &Path) -> Updater<B, Events> {
        Updater::new(backend, Arc::new(Events), dir, "1.0.0")
    }

    struct FailingCheckBackend;
    impl UpdateBackend for FailingCheckBackend {
        fn check(&self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
            Box::pin(async { Err(Error::Backend("check failed".into())) })
        }
        fn download<'a>(
            &'a self,
            _version: &'a str,
            _on_progress: &'a (dyn Fn(u64, Option<u64>) + Send + Sync),
        ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
            Box::pin(async { unreachable!() })
        }
        fn install(&self, _version: &str, _bytes: &[u8]) -> Result<()> {
            unreachable!()
        }
    }

    struct FailingDownloadBackend;
    impl UpdateBackend for FailingDownloadBackend {
        fn check(&self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
            Box::pin(async { Ok(Some("2.0.0".into())) })
        }
        fn download<'a>(
            &'a self,
            _version: &'a str,
            _on_progress: &'a (dyn Fn(u64, Option<u64>) + Send + Sync),
        ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
            Box::pin(async { Err(Error::Backend("download failed".into())) })
        }
        fn install(&self, _version: &str, _bytes: &[u8]) -> Result<()> {
            unreachable!()
        }
    }

    struct MeetingDuringCheckBackend {
        meeting: Arc<AtomicBool>,
        installs: AtomicUsize,
    }

    impl UpdateBackend for MeetingDuringCheckBackend {
        fn check(&self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
            self.meeting.store(true, Ordering::Relaxed);
            Box::pin(async { Ok(Some("2.0.0".into())) })
        }

        fn download<'a>(
            &'a self,
            _version: &'a str,
            _on_progress: &'a (dyn Fn(u64, Option<u64>) + Send + Sync),
        ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
            Box::pin(async { Ok(b"update".to_vec()) })
        }

        fn install(&self, _version: &str, _bytes: &[u8]) -> Result<()> {
            self.installs.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    struct MeetingDuringDownloadBackend {
        meeting: Arc<AtomicBool>,
        installs: AtomicUsize,
    }

    impl UpdateBackend for MeetingDuringDownloadBackend {
        fn check(&self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
            Box::pin(async { Ok(Some("2.0.0".into())) })
        }

        fn download<'a>(
            &'a self,
            _version: &'a str,
            _on_progress: &'a (dyn Fn(u64, Option<u64>) + Send + Sync),
        ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
            self.meeting.store(true, Ordering::Relaxed);
            Box::pin(async { Ok(b"update".to_vec()) })
        }

        fn install(&self, _version: &str, _bytes: &[u8]) -> Result<()> {
            self.installs.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    struct UnsupportedInstallBackend {
        installs: AtomicUsize,
    }

    impl UpdateBackend for UnsupportedInstallBackend {
        fn check(&self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
            Box::pin(async { Ok(Some("2.0.0".into())) })
        }

        fn download<'a>(
            &'a self,
            _version: &'a str,
            _on_progress: &'a (dyn Fn(u64, Option<u64>) + Send + Sync),
        ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
            Box::pin(async { Ok(b"update".to_vec()) })
        }

        fn install(&self, _version: &str, _bytes: &[u8]) -> Result<()> {
            self.installs.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn supports_install(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn meeting_starting_during_check_defers_install() {
        let dir = tempfile::tempdir().unwrap();
        let meeting = Arc::new(AtomicBool::new(false));
        let backend = Arc::new(MeetingDuringCheckBackend {
            meeting: meeting.clone(),
            installs: AtomicUsize::new(0),
        });
        let policy = || UpdatePolicy {
            automatic_updates_enabled: true,
            meeting_active: meeting.load(Ordering::Relaxed),
        };
        assert!(
            updater_with_backend(backend.clone(), dir.path())
                .tick(&policy, true)
                .await
        );
        assert_eq!(backend.installs.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn meeting_starting_during_download_defers_install() {
        let dir = tempfile::tempdir().unwrap();
        let meeting = Arc::new(AtomicBool::new(false));
        let backend = Arc::new(MeetingDuringDownloadBackend {
            meeting: meeting.clone(),
            installs: AtomicUsize::new(0),
        });
        let policy = || UpdatePolicy {
            automatic_updates_enabled: true,
            meeting_active: meeting.load(Ordering::Relaxed),
        };
        assert!(
            updater_with_backend(backend.clone(), dir.path())
                .tick(&policy, true)
                .await
        );
        assert_eq!(backend.installs.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn unsupported_install_downloads_without_installing() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(UnsupportedInstallBackend {
            installs: AtomicUsize::new(0),
        });
        let policy = || UpdatePolicy {
            automatic_updates_enabled: true,
            meeting_active: false,
        };
        assert!(
            !updater_with_backend(backend.clone(), dir.path())
                .tick(&policy, true)
                .await
        );
        assert_eq!(backend.installs.load(Ordering::Relaxed), 0);
        assert!(cache_path(dir.path(), "2.0.0").is_file());
    }

    #[test]
    fn prune_behaviour() {
        let dir = tempfile::tempdir().unwrap();
        for version in ["1.0.0", "2.0.0"] {
            std::fs::write(dir.path().join(format!("{version}.bin")), b"update").unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), b"keep").unwrap();
        prune_updates_dir(dir.path(), Some("2.0.0"));
        assert!(dir.path().join("2.0.0.bin").exists());
        assert!(!dir.path().join("1.0.0.bin").exists());
        assert!(dir.path().join("notes.txt").exists());
    }
}
