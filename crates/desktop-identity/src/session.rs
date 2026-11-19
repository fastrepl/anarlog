//! Where each platform keeps the sign-in session, and how: the plaintext `auth.json` on macOS,
//! which the bundled CLI also reads; DPAPI-protected `auth.dpapi` on Windows; and Secret Service
//! on Linux, keeping the plaintext file while Secret Service is unreadable and honoring the
//! CLI's `auth.cli.json` fallback. The session is a supabase-js storage map whose session
//! JSON sits under the key ending in [`SESSION_KEY_SUFFIX`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::auth::{AuthPaths, LINUX_SECRET_KEY, LINUX_SECRET_SCOPE, cli_fallback_path};
use crate::chunked::SecretSlot;
use crate::secure_store::SecureStore;

pub type SessionMap = HashMap<String, String>;

pub const SESSION_KEY_SUFFIX: &str = "-auth-token";

/// The key shipping's supabase-js client stores the session under:
/// `sb-<first label of the Supabase host>-auth-token`.
pub fn storage_key(supabase_host: &str) -> String {
    let project = supabase_host.split('.').next().unwrap_or_default();
    format!("sb-{project}{SESSION_KEY_SUFFIX}")
}

/// The secure store holding the Linux session.
pub trait Secrets: Send + Sync {
    fn read(&self, scope: &str, key: &str) -> Result<Option<String>, String>;
    fn write(&self, scope: &str, key: &str, value: &str) -> Result<(), String>;
    fn delete(&self, scope: &str, key: &str) -> Result<(), String>;
}

impl<F, S> Secrets for SecureStore<F>
where
    F: Fn(&str, &str) -> keyring::Result<S> + Send + Sync,
    S: SecretSlot,
{
    fn read(&self, scope: &str, key: &str) -> Result<Option<String>, String> {
        SecureStore::read(self, scope, key)
    }

    fn write(&self, scope: &str, key: &str, value: &str) -> Result<(), String> {
        SecureStore::write(self, scope, key, value)
    }

    fn delete(&self, scope: &str, key: &str) -> Result<(), String> {
        SecureStore::delete(self, scope, key)
    }
}

/// An encrypted session file: DPAPI on Windows.
pub trait Vault: Send + Sync {
    fn load(&self, path: &Path) -> Result<SessionMap, String>;
    fn persist(&self, path: &Path, session: &SessionMap) -> Result<(), String>;
    fn clear(&self, path: &Path) -> Result<(), String>;
}

#[cfg(target_os = "windows")]
pub struct Dpapi;

#[cfg(target_os = "windows")]
impl Vault for Dpapi {
    fn load(&self, path: &Path) -> Result<SessionMap, String> {
        anlg_storage::windows_auth::load(path).map_err(|error| error.to_string())
    }

    fn persist(&self, path: &Path, session: &SessionMap) -> Result<(), String> {
        anlg_storage::windows_auth::persist(path, session).map_err(|error| error.to_string())
    }

    fn clear(&self, path: &Path) -> Result<(), String> {
        anlg_storage::windows_auth::clear(path).map_err(|error| error.to_string())
    }
}

/// One channel's session storage. Calls block on the file system and credential store.
pub struct SessionStore {
    backend: Backend,
}

enum Backend {
    File(PathBuf),
    SecretService {
        paths: AuthPaths,
        secrets: Box<dyn Secrets>,
    },
    Protected {
        paths: AuthPaths,
        vault: Box<dyn Vault>,
    },
}

impl SessionStore {
    /// The backend the shipping app uses on this platform for the channel `identifier`.
    pub fn for_platform(paths: AuthPaths, identifier: &str) -> Self {
        #[cfg(target_os = "linux")]
        let store = Self::secret_service(paths, SecureStore::keyring(identifier));
        #[cfg(target_os = "windows")]
        let store = Self::protected(paths, Dpapi);
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let store = Self::file(paths.resolve());
        #[cfg(not(target_os = "linux"))]
        let _ = identifier;
        store
    }

    pub fn file(path: PathBuf) -> Self {
        Self {
            backend: Backend::File(path),
        }
    }

    pub fn secret_service(paths: AuthPaths, secrets: impl Secrets + 'static) -> Self {
        Self {
            backend: Backend::SecretService {
                paths,
                secrets: Box::new(secrets),
            },
        }
    }

    pub fn protected(paths: AuthPaths, vault: impl Vault + 'static) -> Self {
        Self {
            backend: Backend::Protected {
                paths,
                vault: Box::new(vault),
            },
        }
    }

    /// The stored session map; empty when there is none or it can't be read. Never clears a
    /// session it could not read from a secure store.
    pub fn load(&self) -> SessionMap {
        match &self.backend {
            Backend::File(path) => read_file(path).unwrap_or_default(),
            Backend::SecretService { paths, secrets } => {
                load_secret_service(paths, secrets.as_ref())
            }
            Backend::Protected { paths, vault } => load_protected(paths, vault.as_ref()),
        }
    }

    pub fn save(&self, session: &SessionMap) -> Result<(), String> {
        match &self.backend {
            Backend::File(path) => write_file(path, session),
            Backend::SecretService { paths, secrets } => {
                persist_secret_service(paths, secrets.as_ref(), session)
            }
            Backend::Protected { paths, vault } => {
                vault.persist(&protected_path(paths), session)?;
                discard_protected_plaintext(paths);
                Ok(())
            }
        }
    }

    pub fn clear(&self) -> Result<(), String> {
        match &self.backend {
            Backend::File(path) => match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            },
            Backend::SecretService { paths, secrets } => {
                clear_secret_service(paths, secrets.as_ref())
            }
            Backend::Protected { paths, vault } => {
                discard_protected_plaintext(paths);
                vault.clear(&protected_path(paths))
            }
        }
    }
}

fn read_file(path: &Path) -> Result<SessionMap, String> {
    let data = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&data).map_err(|error| error.to_string())
}

fn write_file(path: &Path, session: &SessionMap) -> Result<(), String> {
    let content = serde_json::to_string(session).map_err(|error| error.to_string())?;
    anlg_storage::fs::atomic_write(path, &content).map_err(|error| error.to_string())
}

// A leftover auth.json is less harmful than refusing a session the secure store
// already holds, so cleanup failures are reported rather than propagated.
fn discard_plaintext(path: &Path) {
    if let Err(error) = crate::auth::remove_plaintext(path) {
        tracing::warn!(
            path = %path.display(),
            %error,
            "failed_to_remove_plaintext_auth"
        );
    }
}

fn load_secret_service(paths: &AuthPaths, secrets: &dyn Secrets) -> SessionMap {
    let auth_path = paths.resolve();
    let cli_fallback_path = cli_fallback_path(&auth_path);

    // A locked or unavailable keyring must not abort startup, otherwise the app
    // cannot start and the plaintext session can never be migrated. `Ok(None)` means
    // there is no secret yet; `Err` only means it could not be read this time, which
    // is a distinction the migration below depends on.
    let secure_read = secrets.read(LINUX_SECRET_SCOPE, LINUX_SECRET_KEY);
    let keyring_readable = secure_read.is_ok();
    let secure_data = match secure_read {
        Ok(data) => data,
        Err(error) => {
            tracing::warn!(%error, "failed_to_read_auth_from_secret_service");
            None
        }
    };

    if cli_fallback_path.is_file() {
        match read_file(&cli_fallback_path) {
            Ok(auth) => {
                if keyring_readable
                    && let Err(error) = persist_secret_service(paths, secrets, &auth)
                {
                    tracing::warn!(%error, "failed_to_reconcile_cli_auth_with_secret_service");
                }
                return auth;
            }
            Err(error) => {
                tracing::warn!(%error, "ignoring_unreadable_cli_auth_fallback");
                discard_plaintext(&cli_fallback_path);
            }
        }
    }

    if let Some(data) = secure_data {
        match serde_json::from_str::<SessionMap>(&data) {
            // Only drop the plaintext copy once the secure payload is known to be usable.
            Ok(auth) => {
                discard_plaintext(&auth_path);
                return auth;
            }
            Err(error) => {
                tracing::warn!(%error, "ignoring_unreadable_secret_service_auth");
            }
        }
    }

    if !auth_path.is_file() {
        return SessionMap::new();
    }

    let auth = match read_file(&auth_path) {
        Ok(auth) => auth,
        Err(error) => {
            // Matches the keyring path: a corrupt or truncated auth.json costs the
            // session, but must not stop the app from launching. Dropping it also
            // breaks the loop where a half-removed file fails to parse every launch.
            tracing::warn!(%error, "ignoring_unreadable_plaintext_auth");
            discard_plaintext(&auth_path);
            return SessionMap::new();
        }
    };

    // Writing now could clobber a secret that exists but could not be read, so an
    // unreadable keyring keeps the plaintext copy and retries on a later launch.
    if !keyring_readable {
        return auth;
    }

    // persist_secret_service drops the plaintext copy itself once the keyring owns it.
    if let Err(error) = persist_secret_service(paths, secrets, &auth) {
        tracing::warn!(%error, "failed_to_migrate_auth_to_secret_service");
    }

    auth
}

fn persist_secret_service(
    paths: &AuthPaths,
    secrets: &dyn Secrets,
    auth: &SessionMap,
) -> Result<(), String> {
    if auth.is_empty() {
        return clear_secret_service(paths, secrets);
    }

    let data = serde_json::to_string(auth).map_err(|error| error.to_string())?;
    secrets.write(LINUX_SECRET_SCOPE, LINUX_SECRET_KEY, &data)?;

    drop_secret_service_plaintext(paths);
    Ok(())
}

fn clear_secret_service(paths: &AuthPaths, secrets: &dyn Secrets) -> Result<(), String> {
    secrets.delete(LINUX_SECRET_SCOPE, LINUX_SECRET_KEY)?;
    drop_secret_service_plaintext(paths);
    Ok(())
}

// The keyring is authoritative once it has been written or cleared, so a plaintext
// copy left behind by a failed migration must not outlive it and silently restore
// the session on the next launch.
fn drop_secret_service_plaintext(paths: &AuthPaths) {
    let path = paths.resolve();
    discard_plaintext(&path);
    discard_plaintext(&cli_fallback_path(&path));
}

fn protected_path(paths: &AuthPaths) -> PathBuf {
    anlg_storage::windows_auth::secure_path(&paths.current)
}

fn load_protected(paths: &AuthPaths, vault: &dyn Vault) -> SessionMap {
    let plaintext_path = paths.resolve();
    let secure_path = protected_path(paths);

    if secure_path.is_file() {
        let auth = match vault.load(&secure_path) {
            Ok(auth) => auth,
            Err(error) => {
                tracing::warn!(%error, "ignoring_unreadable_windows_auth");
                SessionMap::new()
            }
        };
        discard_protected_plaintext(paths);
        return auth;
    }

    if !plaintext_path.is_file() {
        return SessionMap::new();
    }

    let auth = match read_file(&plaintext_path) {
        Ok(auth) => auth,
        Err(error) => {
            tracing::warn!(%error, "ignoring_unreadable_plaintext_auth");
            discard_protected_plaintext(paths);
            return SessionMap::new();
        }
    };

    if let Err(error) = vault.persist(&secure_path, &auth) {
        tracing::warn!(%error, "failed_to_migrate_auth_to_windows_data_protection");
        return auth;
    }

    discard_protected_plaintext(paths);
    auth
}

fn discard_protected_plaintext(paths: &AuthPaths) {
    discard_plaintext(&paths.current);
    discard_plaintext(&paths.legacy);

    if let Err(error) = crate::auth::remove_auth_from_store_json(&paths.legacy_store_json) {
        tracing::warn!(
            path = %paths.legacy_store_json.display(),
            %error,
            "failed_to_remove_auth_from_legacy_store"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use super::*;

    const KEY: &str = "sb-project-auth-token";

    fn session(token: &str) -> SessionMap {
        SessionMap::from([(KEY.to_string(), format!(r#"{{"access_token":"{token}"}}"#))])
    }

    fn paths(root: &Path) -> AuthPaths {
        AuthPaths {
            current: root.join("local/com.hyprnote.stable/auth.json"),
            legacy: root.join("data/anarlog/auth.json"),
            legacy_store_json: root.join("data/anarlog/store.json"),
        }
    }

    fn write_json(path: &Path, session: &SessionMap) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_string(session).unwrap()).unwrap();
    }

    #[test]
    fn storage_key_matches_the_supabase_js_client() {
        assert_eq!(
            storage_key("abcdefgh.supabase.co"),
            "sb-abcdefgh-auth-token"
        );
        assert_eq!(storage_key("localhost"), "sb-localhost-auth-token");
    }

    #[test]
    fn file_sessions_round_trip_in_the_shipping_format() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("auth.json");
        let store = SessionStore::file(path.clone());
        assert!(store.load().is_empty());

        store.save(&session("a")).unwrap();
        let written: SessionMap =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written, session("a"));
        assert_eq!(store.load(), session("a"));

        store.clear().unwrap();
        assert!(!path.exists());
        store.clear().unwrap();
    }

    /// Stands in for Secret Service; reads fail while `unreadable` is set, as when an existing
    /// item can't be decrypted this time.
    #[derive(Clone, Default)]
    struct Keyring {
        items: Arc<Mutex<BTreeMap<String, String>>>,
        unreadable: bool,
    }

    impl Keyring {
        fn stored(&self) -> Option<SessionMap> {
            let items = self.items.lock().unwrap();
            items
                .get("auth:supabase-storage")
                .map(|data| serde_json::from_str(data).unwrap())
        }
    }

    impl Secrets for Keyring {
        fn read(&self, scope: &str, key: &str) -> Result<Option<String>, String> {
            if self.unreadable {
                return Err("locked".into());
            }
            Ok(self
                .items
                .lock()
                .unwrap()
                .get(&format!("{scope}:{key}"))
                .cloned())
        }

        fn write(&self, scope: &str, key: &str, value: &str) -> Result<(), String> {
            let mut items = self.items.lock().unwrap();
            items.insert(format!("{scope}:{key}"), value.to_string());
            Ok(())
        }

        fn delete(&self, scope: &str, key: &str) -> Result<(), String> {
            self.items.lock().unwrap().remove(&format!("{scope}:{key}"));
            Ok(())
        }
    }

    #[test]
    fn secret_service_moves_plaintext_into_the_keyring_only_when_readable() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        write_json(&paths.current, &session("plain"));

        let unreadable = Keyring {
            unreadable: true,
            ..Keyring::default()
        };
        let store = SessionStore::secret_service(paths.clone(), unreadable.clone());
        assert_eq!(store.load(), session("plain"));
        assert!(
            paths.current.is_file(),
            "an unreadable keyring keeps plaintext"
        );
        assert_eq!(unreadable.stored(), None, "and isn't overwritten");

        let keyring = Keyring::default();
        let store = SessionStore::secret_service(paths.clone(), keyring.clone());
        assert_eq!(store.load(), session("plain"));
        assert_eq!(keyring.stored(), Some(session("plain")));
        assert!(!paths.current.exists());
        assert_eq!(store.load(), session("plain"));
    }

    #[test]
    fn secret_service_prefers_and_reconciles_the_cli_fallback() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let keyring = Keyring::default();
        keyring.items.lock().unwrap().insert(
            "auth:supabase-storage".into(),
            serde_json::to_string(&session("old")).unwrap(),
        );
        let fallback = cli_fallback_path(&paths.current);
        write_json(&fallback, &session("cli"));

        let store = SessionStore::secret_service(paths.clone(), keyring.clone());
        assert_eq!(store.load(), session("cli"));
        assert_eq!(keyring.stored(), Some(session("cli")));
        assert!(!fallback.exists());

        std::fs::write(&fallback, "{ torn").unwrap();
        assert_eq!(store.load(), session("cli"));
        assert!(!fallback.exists());
    }

    #[test]
    fn secret_service_saves_and_clears_the_keyring_and_plaintext() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let keyring = Keyring::default();
        let store = SessionStore::secret_service(paths.clone(), keyring.clone());
        write_json(&paths.current, &session("stale"));

        store.save(&session("new")).unwrap();
        assert_eq!(keyring.stored(), Some(session("new")));
        assert!(!paths.current.exists());

        store.save(&SessionMap::new()).unwrap();
        assert_eq!(keyring.stored(), None);

        store.save(&session("new")).unwrap();
        store.clear().unwrap();
        assert_eq!(keyring.stored(), None);
        assert!(store.load().is_empty());
    }

    /// Stands in for DPAPI: a marked JSON file, failing writes while `refuse` is set.
    #[derive(Default)]
    struct FakeVault {
        refuse: bool,
    }

    impl Vault for FakeVault {
        fn load(&self, path: &Path) -> Result<SessionMap, String> {
            let data = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
            let data = data.strip_prefix("protected:").ok_or("unprotected")?;
            serde_json::from_str(data).map_err(|error| error.to_string())
        }

        fn persist(&self, path: &Path, session: &SessionMap) -> Result<(), String> {
            if self.refuse {
                return Err("refused".into());
            }
            let data = format!("protected:{}", serde_json::to_string(session).unwrap());
            anlg_storage::fs::atomic_write(path, &data).map_err(|error| error.to_string())
        }

        fn clear(&self, path: &Path) -> Result<(), String> {
            std::fs::remove_file(path).map_err(|error| error.to_string())
        }
    }

    #[test]
    fn protected_sessions_migrate_plaintext_and_drop_every_copy() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let secure = paths.current.with_file_name("auth.dpapi");
        write_json(&paths.current, &session("plain"));

        let refusing = SessionStore::protected(paths.clone(), FakeVault { refuse: true });
        assert_eq!(refusing.load(), session("plain"));
        assert!(
            paths.current.is_file(),
            "a failed migration keeps plaintext"
        );

        let store = SessionStore::protected(paths.clone(), FakeVault::default());
        assert_eq!(store.load(), session("plain"));
        assert!(secure.is_file());
        assert!(!paths.current.exists());

        write_json(&paths.legacy, &session("legacy"));
        std::fs::write(&paths.legacy_store_json, r#"{"auth":"{}","desktop":"{}"}"#).unwrap();
        assert_eq!(store.load(), session("plain"));
        assert!(!paths.legacy.exists());
        assert_eq!(
            std::fs::read_to_string(&paths.legacy_store_json).unwrap(),
            r#"{"desktop":"{}"}"#
        );

        store.save(&session("new")).unwrap();
        assert_eq!(store.load(), session("new"));
        store.clear().unwrap();
        assert!(!secure.exists());
        assert!(store.load().is_empty());
    }

    #[test]
    fn unreadable_protected_sessions_read_as_signed_out() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let secure = paths.current.with_file_name("auth.dpapi");
        std::fs::create_dir_all(secure.parent().unwrap()).unwrap();
        std::fs::write(&secure, "garbage").unwrap();
        write_json(&paths.current, &session("plain"));

        let store = SessionStore::protected(paths.clone(), FakeVault::default());
        assert!(store.load().is_empty());
        assert!(!paths.current.exists());
        assert!(secure.is_file());
    }
}
