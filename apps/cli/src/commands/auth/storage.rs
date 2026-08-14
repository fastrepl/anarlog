use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anlg_supabase_auth::session::Session;

use crate::{Error, Result};

#[cfg(target_os = "linux")]
const SECRET_SERVICE: &str = "com.anarlog.stable.secure-store";
#[cfg(target_os = "linux")]
const SECRET_ACCOUNT: &str = "auth:supabase-storage";
#[cfg(target_os = "linux")]
const CLI_FALLBACK_FILENAME: &str = "auth.cli.json";

#[derive(Clone, Copy)]
pub(super) enum Backend {
    #[cfg(target_os = "linux")]
    SecretService,
    #[cfg(target_os = "windows")]
    WindowsDataProtection,
    File,
}

impl Backend {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            #[cfg(target_os = "linux")]
            Self::SecretService => "secret_service",
            #[cfg(target_os = "windows")]
            Self::WindowsDataProtection => "windows_data_protection",
            Self::File => "file",
        }
    }
}

pub(super) struct LoadedAuth {
    pub(super) data: HashMap<String, String>,
    pub(super) backend: Backend,
    #[cfg(any(target_os = "linux", test))]
    source_is_authoritative: bool,
}

pub(super) struct AuthStore {
    path: PathBuf,
    #[cfg(target_os = "linux")]
    use_secret_service: bool,
    #[cfg(target_os = "windows")]
    use_windows_data_protection: bool,
}

impl AuthStore {
    pub(super) fn new(bundle_id: &str) -> Result<Self> {
        let override_path = std::env::var_os("ANARLOG_AUTH_PATH").map(PathBuf::from);
        if override_path
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
        {
            return Err(Error::operation(
                "resolve auth storage",
                "ANARLOG_AUTH_PATH must be an absolute path",
            ));
        }
        let path = match override_path.as_ref() {
            Some(path) => path.clone(),
            None => dirs::data_local_dir()
                .ok_or_else(|| {
                    Error::operation(
                        "resolve auth storage",
                        "local data directory is unavailable",
                    )
                })?
                .join(bundle_id)
                .join("auth.json"),
        };
        #[cfg(target_os = "linux")]
        let use_secret_service = override_path.is_none() && bundle_id == "com.hyprnote.stable";
        #[cfg(target_os = "linux")]
        let path = if use_secret_service {
            path.with_file_name(CLI_FALLBACK_FILENAME)
        } else {
            path
        };
        #[cfg(target_os = "windows")]
        let use_windows_data_protection = override_path.is_none();
        #[cfg(target_os = "windows")]
        let path = if use_windows_data_protection {
            anlg_storage::windows_auth::secure_path(&path)
        } else {
            path
        };
        Ok(Self {
            path,
            #[cfg(target_os = "linux")]
            use_secret_service,
            #[cfg(target_os = "windows")]
            use_windows_data_protection,
        })
    }

    #[cfg(test)]
    fn at(path: PathBuf) -> Self {
        Self {
            path,
            #[cfg(target_os = "linux")]
            use_secret_service: false,
            #[cfg(target_os = "windows")]
            use_windows_data_protection: false,
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn load(&self) -> Result<LoadedAuth> {
        #[cfg(target_os = "windows")]
        if self.use_windows_data_protection {
            let data = match anlg_storage::windows_auth::load(&self.path) {
                Ok(data) => data,
                Err(anlg_storage::Error::Io(error))
                    if error.kind() == std::io::ErrorKind::NotFound =>
                {
                    HashMap::new()
                }
                Err(error) => {
                    return Err(Error::operation("read auth storage", error.to_string()));
                }
            };
            return Ok(LoadedAuth {
                data,
                backend: Backend::WindowsDataProtection,
                #[cfg(any(target_os = "linux", test))]
                source_is_authoritative: true,
            });
        }

        #[cfg(target_os = "linux")]
        if self.use_secret_service {
            if let Some(data) = self.read_fallback() {
                if save_secret_service(&data).is_ok() {
                    let _ = self.remove_file();
                    return Ok(LoadedAuth {
                        data,
                        backend: Backend::SecretService,
                        #[cfg(any(target_os = "linux", test))]
                        source_is_authoritative: true,
                    });
                }
                return Ok(LoadedAuth {
                    data,
                    backend: Backend::File,
                    #[cfg(any(target_os = "linux", test))]
                    source_is_authoritative: true,
                });
            }

            let source_is_authoritative = match load_secret_service() {
                Ok(Some(data)) => {
                    return Ok(LoadedAuth {
                        data,
                        backend: Backend::SecretService,
                        #[cfg(any(target_os = "linux", test))]
                        source_is_authoritative: true,
                    });
                }
                Ok(None) => true,
                Err(SecretReadError::Unavailable) => false,
                Err(SecretReadError::Invalid(reason)) => {
                    return Err(Error::operation("read auth storage", reason));
                }
            };

            return Ok(LoadedAuth {
                data: HashMap::new(),
                backend: Backend::File,
                #[cfg(any(target_os = "linux", test))]
                source_is_authoritative,
            });
        }

        Ok(LoadedAuth {
            data: self.read_file()?.unwrap_or_default(),
            backend: Backend::File,
            #[cfg(any(target_os = "linux", test))]
            source_is_authoritative: true,
        })
    }

    pub(super) fn save(&self, data: &HashMap<String, String>) -> Result<Backend> {
        #[cfg(target_os = "windows")]
        if self.use_windows_data_protection {
            anlg_storage::windows_auth::persist(&self.path, data)
                .map_err(|error| Error::operation("write auth storage", error.to_string()))?;
            return Ok(Backend::WindowsDataProtection);
        }

        #[cfg(target_os = "linux")]
        if self.use_secret_service {
            // Keep an existing fallback current before updating Secret Service so a
            // cleanup failure cannot restore the previous session later.
            let fallback_staged = self.stage_fallback(data)?;
            if save_secret_service(data).is_ok() {
                let _ = self.remove_file();
                return Ok(Backend::SecretService);
            }
            if !fallback_staged {
                self.write_file(data)?;
            }
            return Ok(Backend::File);
        }

        self.write_file(data)?;
        Ok(Backend::File)
    }

    pub(super) fn remove_sessions(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        if self.use_windows_data_protection {
            let mut loaded = self.load()?;
            loaded.data.retain(|key, _| !key.ends_with("-auth-token"));
            if loaded.data.is_empty() {
                anlg_storage::windows_auth::clear(&self.path)
                    .map_err(|error| Error::operation("clear auth storage", error.to_string()))?;
            } else {
                anlg_storage::windows_auth::persist(&self.path, &loaded.data)
                    .map_err(|error| Error::operation("clear auth storage", error.to_string()))?;
            }
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        if self.use_secret_service {
            return self.remove_loaded_sessions(self.load()?);
        }

        if let Some(mut data) = self.read_file()? {
            data.retain(|key, _| !key.ends_with("-auth-token"));
            if data.is_empty() {
                self.remove_file()?;
            } else {
                self.write_file(&data)?;
            }
        }
        Ok(())
    }

    #[cfg(any(target_os = "linux", test))]
    fn remove_loaded_sessions(&self, mut loaded: LoadedAuth) -> Result<()> {
        if !loaded.source_is_authoritative {
            return Ok(());
        }
        loaded.data.retain(|key, _| !key.ends_with("-auth-token"));
        self.save(&loaded.data)?;
        Ok(())
    }

    fn read_file(&self) -> Result<Option<HashMap<String, String>>> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::operation("read auth storage", error.to_string())),
        };
        serde_json::from_str(&content)
            .map(Some)
            .map_err(|error| Error::operation("read auth storage", error.to_string()))
    }

    #[cfg(any(target_os = "linux", test))]
    fn read_fallback(&self) -> Option<HashMap<String, String>> {
        match self.read_file() {
            Ok(data) => data,
            Err(_) => {
                // A malformed fallback has no recoverable state and must not hide a
                // usable Secret Service session.
                let _ = self.remove_file();
                None
            }
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn stage_fallback(&self, data: &HashMap<String, String>) -> Result<bool> {
        if !self.path.is_file() {
            return Ok(false);
        }
        self.write_file(data)?;
        Ok(true)
    }

    fn write_file(&self, data: &HashMap<String, String>) -> Result<()> {
        let content = serde_json::to_string(data)
            .map_err(|error| Error::operation("serialize auth storage", error.to_string()))?;
        anlg_storage::fs::atomic_write(&self.path, &content)
            .map_err(|error| Error::operation("write auth storage", error.to_string()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))
                .map_err(|error| Error::operation("protect auth storage", error.to_string()))?;
        }
        Ok(())
    }

    fn remove_file(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::operation("clear auth storage", error.to_string())),
        }
    }
}

pub(super) fn find_session(data: &HashMap<String, String>) -> Result<Option<Session>> {
    data.iter()
        .filter(|(key, _)| key.ends_with("-auth-token"))
        .map(|(_, value)| {
            serde_json::from_str::<Session>(value)
                .map_err(|error| Error::operation("read login", error.to_string()))
        })
        .collect::<Result<Vec<_>>>()
        .map(|sessions| {
            sessions
                .into_iter()
                .max_by_key(|session| session.expires_at.unwrap_or(0))
        })
}

#[cfg(target_os = "linux")]
enum SecretReadError {
    Unavailable,
    Invalid(String),
}

#[cfg(target_os = "linux")]
fn load_secret_service() -> std::result::Result<Option<HashMap<String, String>>, SecretReadError> {
    let entry = keyring::Entry::new(SECRET_SERVICE, SECRET_ACCOUNT)
        .map_err(|_| SecretReadError::Unavailable)?;
    match entry.get_password() {
        Ok(value) => serde_json::from_str(&value)
            .map(Some)
            .map_err(|error| SecretReadError::Invalid(error.to_string())),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(SecretReadError::Unavailable),
    }
}

#[cfg(target_os = "linux")]
fn save_secret_service(data: &HashMap<String, String>) -> std::result::Result<(), String> {
    let entry =
        keyring::Entry::new(SECRET_SERVICE, SECRET_ACCOUNT).map_err(|error| error.to_string())?;
    if data.is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        };
    }
    let value = serde_json::to_string(data).map_err(|error| error.to_string())?;
    entry
        .set_password(&value)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn session_json(id: &str, expires_at: u64) -> String {
        serde_json::json!({
            "access_token": "access",
            "refresh_token": "refresh",
            "token_type": "bearer",
            "expires_at": expires_at,
            "user": { "id": id, "email": format!("{id}@example.com") }
        })
        .to_string()
    }

    #[test]
    fn file_store_round_trips_desktop_auth_shape() {
        let dir = tempdir().unwrap();
        let store = AuthStore::at(dir.path().join("com.hyprnote.stable/auth.json"));
        let data = HashMap::from([(
            "sb-project-auth-token".to_string(),
            session_json("user-1", 100),
        )]);

        assert!(matches!(store.save(&data).unwrap(), Backend::File));
        let loaded = store.load().unwrap();
        assert_eq!(loaded.data, data);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(store.path())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn logout_removes_only_auth_sessions() {
        let dir = tempdir().unwrap();
        let store = AuthStore::at(dir.path().join("auth.json"));
        let data = HashMap::from([
            (
                "sb-project-auth-token".to_string(),
                session_json("user-1", 100),
            ),
            ("unrelated".to_string(), "value".to_string()),
        ]);
        store.save(&data).unwrap();

        store.remove_sessions().unwrap();

        assert_eq!(
            store.load().unwrap().data,
            HashMap::from([("unrelated".to_string(), "value".to_string())])
        );
    }

    #[test]
    fn corrupt_fallback_is_discarded() {
        let dir = tempdir().unwrap();
        let store = AuthStore::at(dir.path().join("auth.cli.json"));
        std::fs::write(store.path(), "{").unwrap();

        assert!(store.read_fallback().is_none());
        assert!(!store.path().exists());
    }

    #[test]
    fn existing_fallback_is_updated_before_secure_storage() {
        let dir = tempdir().unwrap();
        let store = AuthStore::at(dir.path().join("auth.cli.json"));
        let old_data = HashMap::from([(
            "sb-project-auth-token".to_string(),
            session_json("old", 100),
        )]);
        let new_data = HashMap::from([(
            "sb-project-auth-token".to_string(),
            session_json("new", 200),
        )]);
        store.write_file(&old_data).unwrap();

        assert!(store.stage_fallback(&new_data).unwrap());
        assert_eq!(store.read_file().unwrap(), Some(new_data));
    }

    #[test]
    fn logout_without_authoritative_state_does_not_create_fallback() {
        let dir = tempdir().unwrap();
        let store = AuthStore::at(dir.path().join("auth.cli.json"));
        let loaded = LoadedAuth {
            data: HashMap::new(),
            backend: Backend::File,
            source_is_authoritative: false,
        };

        store.remove_loaded_sessions(loaded).unwrap();

        assert!(!store.path().exists());
    }

    #[test]
    fn finds_the_newest_desktop_session() {
        let data = HashMap::from([
            ("sb-old-auth-token".to_string(), session_json("old", 100)),
            ("sb-new-auth-token".to_string(), session_json("new", 200)),
        ]);

        let session = find_session(&data).unwrap().unwrap();

        assert_eq!(session.user.unwrap().id, "new");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_store_round_trips_desktop_dpapi_format() {
        let dir = tempdir().unwrap();
        let store = AuthStore {
            path: dir.path().join("auth.dpapi"),
            use_windows_data_protection: true,
        };
        let data = HashMap::from([(
            "sb-project-auth-token".to_string(),
            session_json("user-1", 100),
        )]);

        assert!(matches!(
            store.save(&data).unwrap(),
            Backend::WindowsDataProtection
        ));
        assert_eq!(store.load().unwrap().data, data);
        assert!(
            !std::fs::read_to_string(store.path())
                .unwrap()
                .contains("access_token")
        );
    }
}
