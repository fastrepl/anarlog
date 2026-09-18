use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{Error, Persistence, Result};
use crate::paths::{cli_fallback_auth_path, discard_plaintext_auth};

pub trait SecretStore: Send + Sync {
    fn read(&self) -> std::result::Result<Option<String>, String>;
    fn write(&self, value: &str) -> std::result::Result<(), String>;
    fn delete(&self) -> std::result::Result<(), String>;
}

pub struct LinuxSecurePersistence {
    secret: Box<dyn SecretStore>,
    auth_path: PathBuf,
}

impl LinuxSecurePersistence {
    pub fn new(secret: Box<dyn SecretStore>, auth_path: PathBuf) -> Self {
        Self { secret, auth_path }
    }

    fn cli_path(&self) -> PathBuf {
        cli_fallback_auth_path(&self.auth_path)
    }

    fn drop_plaintext(&self) {
        discard_plaintext_auth(&self.auth_path).ok();
        discard_plaintext_auth(&self.cli_path()).ok();
    }

    fn read_file(path: &Path) -> std::io::Result<HashMap<String, String>> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }

    fn save_secret(&self, auth: &HashMap<String, String>) -> Result<()> {
        let value = serde_json::to_string(auth)?;
        self.secret.write(&value).map_err(Error::Persistence)?;
        self.drop_plaintext();
        Ok(())
    }
}

impl Persistence for LinuxSecurePersistence {
    fn load(&self) -> Result<HashMap<String, String>> {
        let secure_read = self.secret.read();
        let keyring_readable = secure_read.is_ok();
        let secure_data = match secure_read {
            Ok(data) => data,
            Err(error) => {
                tracing::warn!(%error, "failed_to_read_auth_from_secret_service");
                None
            }
        };

        let cli_path = self.cli_path();
        if cli_path.is_file() {
            match Self::read_file(&cli_path) {
                Ok(auth) => {
                    if keyring_readable && let Err(error) = self.save_secret(&auth) {
                        tracing::warn!(%error, "failed_to_reconcile_cli_auth_with_secret_service");
                    }
                    return Ok(auth);
                }
                Err(error) => {
                    tracing::warn!(%error, "ignoring_unreadable_cli_auth_fallback");
                    discard_plaintext_auth(&cli_path).ok();
                }
            }
        }

        if let Some(data) = secure_data {
            match serde_json::from_str::<HashMap<String, String>>(&data) {
                Ok(auth) => {
                    discard_plaintext_auth(&self.auth_path).ok();
                    return Ok(auth);
                }
                Err(error) => {
                    tracing::warn!(%error, "ignoring_unreadable_secret_service_auth");
                }
            }
        }

        if !self.auth_path.is_file() {
            return Ok(HashMap::new());
        }

        let auth = match Self::read_file(&self.auth_path) {
            Ok(auth) => auth,
            Err(error) => {
                tracing::warn!(%error, "ignoring_unreadable_plaintext_auth");
                discard_plaintext_auth(&self.auth_path).ok();
                return Ok(HashMap::new());
            }
        };

        if !keyring_readable {
            return Ok(auth);
        }

        if let Err(error) = self.save_secret(&auth) {
            tracing::warn!(%error, "failed_to_migrate_auth_to_secret_service");
        }
        Ok(auth)
    }

    fn save(&self, data: &HashMap<String, String>) -> Result<()> {
        if data.is_empty() {
            return self.clear();
        }
        self.save_secret(data)
    }

    fn clear(&self) -> Result<()> {
        self.secret.delete().map_err(Error::Persistence)?;
        self.drop_plaintext();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MemorySecret {
        value: Arc<Mutex<Option<String>>>,
        readable: bool,
        writable: bool,
    }

    impl SecretStore for MemorySecret {
        fn read(&self) -> std::result::Result<Option<String>, String> {
            if !self.readable {
                return Err("unreadable".to_string());
            }
            Ok(self.value.lock().unwrap().clone())
        }

        fn write(&self, value: &str) -> std::result::Result<(), String> {
            if !self.writable {
                return Err("unwritable".to_string());
            }
            *self.value.lock().unwrap() = Some(value.to_string());
            Ok(())
        }

        fn delete(&self) -> std::result::Result<(), String> {
            *self.value.lock().unwrap() = None;
            Ok(())
        }
    }

    fn persistence(temp: &tempfile::TempDir, secret: MemorySecret) -> LinuxSecurePersistence {
        LinuxSecurePersistence::new(Box::new(secret), temp.path().join("auth.json"))
    }

    #[test]
    fn unreadable_keyring_keeps_plaintext_without_writing() {
        let temp = tempfile::tempdir().unwrap();
        let secret = MemorySecret {
            readable: false,
            writable: true,
            ..Default::default()
        };
        let persistence = persistence(&temp, secret.clone());
        std::fs::write(temp.path().join("auth.json"), r#"{"session":"plaintext"}"#).unwrap();

        let loaded = persistence.load().unwrap();
        assert_eq!(loaded["session"], "plaintext");
        assert!(secret.value.lock().unwrap().is_none());
        assert!(temp.path().join("auth.json").exists());
    }

    #[test]
    fn readable_keyring_migrates_plaintext_and_removes_file() {
        let temp = tempfile::tempdir().unwrap();
        let secret = MemorySecret {
            readable: true,
            writable: true,
            ..Default::default()
        };
        let persistence = persistence(&temp, secret.clone());
        std::fs::write(temp.path().join("auth.json"), r#"{"session":"plaintext"}"#).unwrap();

        persistence.load().unwrap();
        assert!(secret.value.lock().unwrap().is_some());
        assert!(!temp.path().join("auth.json").exists());
    }

    #[test]
    fn failed_keyring_save_does_not_create_plaintext() {
        let temp = tempfile::tempdir().unwrap();
        let secret = MemorySecret {
            readable: true,
            writable: false,
            ..Default::default()
        };
        let persistence = persistence(&temp, secret);
        let mut data = HashMap::new();
        data.insert("session".to_string(), "value".to_string());

        assert!(persistence.save(&data).is_err());
        assert!(!temp.path().join("auth.json").exists());
    }

    #[test]
    fn cli_fallback_wins_and_is_reconciled() {
        let temp = tempfile::tempdir().unwrap();
        let secret = MemorySecret {
            readable: true,
            writable: true,
            ..Default::default()
        };
        let persistence = persistence(&temp, secret.clone());
        std::fs::write(
            temp.path().join(crate::paths::CLI_FALLBACK_FILENAME),
            r#"{"session":"cli"}"#,
        )
        .unwrap();

        let loaded = persistence.load().unwrap();
        assert_eq!(loaded["session"], "cli");
        assert!(secret.value.lock().unwrap().is_some());
        assert!(
            !temp
                .path()
                .join(crate::paths::CLI_FALLBACK_FILENAME)
                .exists()
        );
    }
}
