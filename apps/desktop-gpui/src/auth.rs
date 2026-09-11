use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use anlg_deeplink_core::AuthCallbackSearch;
use anlg_desktop_auth::{AccountInfo, Persistence, SessionManager, paths, storage_key};
#[cfg(target_os = "linux")]
use anlg_desktop_auth::{LinuxSecurePersistence, SecretStore};
use tokio::sync::watch;

const AUTH_SCOPE: &str = "auth";
const AUTH_KEY: &str = "supabase-storage";
const SUPABASE_URL: Option<&str> = option_env!("VITE_SUPABASE_URL");
const SUPABASE_ANON_KEY: Option<&str> = option_env!("VITE_SUPABASE_ANON_KEY");

pub struct Auth {
    session: Arc<SessionManager>,
    callbacks: Mutex<CallbackDeduper>,
    signed_in: watch::Sender<bool>,
}

impl Auth {
    pub fn new(identifier: &str) -> Self {
        let key = SUPABASE_URL.map_or_else(|| "sb-auth-auth-token".to_string(), storage_key);
        let client = SUPABASE_URL
            .zip(SUPABASE_ANON_KEY)
            .map(|(url, anon_key)| anlg_supabase_auth::refresh::AuthClient::new(url, anon_key));
        let persistence = persistence(identifier);
        let session =
            SessionManager::new(key.clone(), persistence, client.clone()).unwrap_or_else(|error| {
                tracing::warn!(%error, "failed to load desktop auth persistence");
                SessionManager::in_memory(key, HashMap::new(), client)
            });
        let signed_in = session.session().ok().flatten().is_some();
        let (signed_in_tx, _) = watch::channel(signed_in);
        Self {
            session: Arc::new(session),
            callbacks: Mutex::new(CallbackDeduper::default()),
            signed_in: signed_in_tx,
        }
    }

    pub fn start(identifier: &str, runtime: &tokio::runtime::Handle) -> Arc<Self> {
        let auth = Arc::new(Self::new(identifier));
        let refresh_auth = auth.clone();
        runtime.spawn(async move {
            loop {
                refresh_auth.refresh().await;
                tokio::time::sleep(Duration::from_secs(300)).await;
            }
        });
        auth
    }

    pub fn signed_in(&self) -> bool {
        *self.signed_in.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.signed_in.subscribe()
    }

    pub fn account_info(&self) -> Option<AccountInfo> {
        self.session.account_info().ok().flatten()
    }

    pub fn sign_out(&self) -> Result<(), String> {
        let result = self.session.sign_out().map_err(|error| error.to_string());
        self.signed_in.send_replace(false);
        result
    }

    pub async fn refresh(&self) {
        if let Err(error) = self.session.ensure_fresh(Duration::from_secs(60)).await {
            tracing::warn!(%error, "failed to refresh desktop auth session");
            if matches!(error, anlg_desktop_auth::Error::Refresh(error) if error.is_fatal())
                && let Err(sign_out_error) = self.sign_out()
            {
                tracing::error!(error = %sign_out_error, "failed to sign out after fatal auth refresh error");
            }
        }
        let signed_in = self.session.session().ok().flatten().is_some();
        self.signed_in.send_replace(signed_in);
    }

    pub async fn handle_callback(
        &self,
        callback: AuthCallbackSearch,
    ) -> Result<CallbackOutcome, String> {
        let fingerprint = format!("{}:{}", callback.access_token, callback.refresh_token);
        if !self
            .callbacks
            .lock()
            .unwrap()
            .begin(&fingerprint, SystemTime::now())
        {
            return Ok(CallbackOutcome::Duplicate);
        }
        let result = self
            .session
            .install_tokens(&callback.access_token, &callback.refresh_token)
            .await
            .map(|_| {
                self.signed_in.send_replace(true);
                CallbackOutcome::Installed
            })
            .map_err(|error| error.to_string());
        self.callbacks
            .lock()
            .unwrap()
            .finish(&fingerprint, SystemTime::now(), result.is_ok());
        result
    }
}

pub enum CallbackOutcome {
    Installed,
    Duplicate,
}

#[derive(Default)]
struct CallbackDeduper {
    recent: Option<(String, SystemTime)>,
    in_flight: Option<String>,
}

impl CallbackDeduper {
    fn begin(&mut self, fingerprint: &str, now: SystemTime) -> bool {
        if self.in_flight.as_deref() == Some(fingerprint) {
            return false;
        }
        if self.recent.as_ref().is_some_and(|(value, at)| {
            value == fingerprint
                && now.duration_since(*at).unwrap_or_default() < Duration::from_secs(5)
        }) {
            return false;
        }
        self.in_flight = Some(fingerprint.to_string());
        true
    }

    fn finish(&mut self, fingerprint: &str, now: SystemTime, succeeded: bool) {
        if self.in_flight.as_deref() == Some(fingerprint) {
            self.in_flight = None;
            if succeeded {
                self.recent = Some((fingerprint.to_string(), now));
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
struct FilePersistence {
    path: PathBuf,
}

#[cfg(not(target_os = "linux"))]
impl Persistence for FilePersistence {
    fn load(&self) -> anlg_desktop_auth::Result<HashMap<String, String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => Ok(serde_json::from_str(&content).unwrap_or_default()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(error) => Err(anlg_desktop_auth::Error::Persistence(error.to_string())),
        }
    }

    fn save(&self, data: &HashMap<String, String>) -> anlg_desktop_auth::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()))?;
        }
        let content = serde_json::to_string(data)?;
        anlg_storage::fs::atomic_write(&self.path, &content)
            .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()))
    }

    fn clear(&self) -> anlg_desktop_auth::Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
struct GpuiSecretStore {
    app_id: String,
}

#[cfg(target_os = "windows")]
struct WindowsPersistence {
    secure_path: PathBuf,
    fallback: FilePersistence,
}

#[cfg(target_os = "windows")]
impl Persistence for WindowsPersistence {
    fn load(&self) -> anlg_desktop_auth::Result<HashMap<String, String>> {
        if self.secure_path.is_file() {
            return anlg_storage::windows_auth::load(&self.secure_path)
                .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()));
        }
        self.fallback.load()
    }

    fn save(&self, data: &HashMap<String, String>) -> anlg_desktop_auth::Result<()> {
        anlg_storage::windows_auth::persist(&self.secure_path, data)
            .map_err(|error| anlg_desktop_auth::Error::Persistence(error.to_string()))
    }

    fn clear(&self) -> anlg_desktop_auth::Result<()> {
        let secure = anlg_storage::windows_auth::clear(&self.secure_path);
        let fallback = self.fallback.clear();
        match (secure, fallback) {
            (Err(error), _) => Err(anlg_desktop_auth::Error::Persistence(error.to_string())),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}

#[cfg(target_os = "linux")]
impl SecretStore for GpuiSecretStore {
    fn read(&self) -> std::result::Result<Option<String>, String> {
        crate::secrets::read(&self.app_id, AUTH_SCOPE, AUTH_KEY)
    }

    fn write(&self, value: &str) -> std::result::Result<(), String> {
        crate::secrets::write(&self.app_id, AUTH_SCOPE, AUTH_KEY, value)
    }

    fn delete(&self) -> std::result::Result<(), String> {
        crate::secrets::delete(&self.app_id, AUTH_SCOPE, AUTH_KEY)
    }
}

#[cfg(target_os = "linux")]
fn linux_persistence(identifier: &str, path: PathBuf) -> LinuxSecurePersistence {
    LinuxSecurePersistence::new(
        Box::new(GpuiSecretStore {
            app_id: identifier.to_string(),
        }),
        path,
    )
}

#[cfg(target_os = "linux")]
fn persistence(identifier: &str) -> Box<dyn Persistence> {
    let data_dir = dirs::data_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
    let local_dir = dirs::data_local_dir().unwrap_or_else(|| data_dir.clone());
    let new_path = local_dir.join(identifier).join(paths::FILENAME);
    let legacy_base = anlg_storage::global::compute_default_base(identifier)
        .unwrap_or_else(|| data_dir.join(identifier));
    let legacy_path = legacy_base.join(paths::FILENAME);
    let store_path = legacy_base.join("store.json");
    let path = paths::resolve_auth_path_from_paths(&legacy_path, &store_path, &new_path);
    Box::new(linux_persistence(identifier, path))
}

#[cfg(not(target_os = "linux"))]
fn persistence(identifier: &str) -> Box<dyn Persistence> {
    let data_dir = dirs::data_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
    let local_dir = dirs::data_local_dir().unwrap_or_else(|| data_dir.clone());
    let new_path = local_dir.join(identifier).join(paths::FILENAME);
    let legacy_base = anlg_storage::global::compute_default_base(identifier)
        .unwrap_or_else(|| data_dir.join(identifier));
    let legacy_path = legacy_base.join(paths::FILENAME);
    let store_path = legacy_base.join("store.json");
    let path = paths::resolve_auth_path_from_paths(&legacy_path, &store_path, &new_path);
    #[cfg(target_os = "windows")]
    {
        return Box::new(WindowsPersistence {
            secure_path: anlg_storage::windows_auth::secure_path(&path),
            fallback: FilePersistence { path },
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        Box::new(FilePersistence { path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupes_for_five_seconds() {
        let mut deduper = CallbackDeduper::default();
        let now = SystemTime::UNIX_EPOCH;
        assert!(deduper.begin("token", now));
        assert!(!deduper.begin("token", now + Duration::from_secs(1)));
        deduper.finish("token", now, true);
        assert!(!deduper.begin("token", now + Duration::from_secs(4)));
        assert!(deduper.begin("token", now + Duration::from_secs(5)));
    }

    #[test]
    fn failed_callback_can_retry_within_five_seconds() {
        let mut deduper = CallbackDeduper::default();
        let now = SystemTime::UNIX_EPOCH;
        assert!(deduper.begin("token", now));
        deduper.finish("token", now, false);
        assert!(deduper.begin("token", now + Duration::from_secs(1)));
    }

    #[test]
    fn successful_callback_is_recent() {
        let mut deduper = CallbackDeduper::default();
        let now = SystemTime::UNIX_EPOCH;
        assert!(deduper.begin("token", now));
        deduper.finish("token", now, true);
        assert!(!deduper.begin("token", now + Duration::from_secs(1)));
    }
}
