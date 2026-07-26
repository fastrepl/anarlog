use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// In-memory state is the source of truth, not the file.
/// This lets the session survive disk write failures.
pub struct AuthStore {
    path: Option<PathBuf>,
    data: Mutex<HashMap<String, String>>,
}

impl AuthStore {
    pub fn load(path: PathBuf) -> Self {
        Self::from_data(path.clone(), load_data(&path))
    }

    pub fn from_data(path: PathBuf, data: HashMap<String, String>) -> Self {
        Self {
            path: Some(path),
            data: Mutex::new(data),
        }
    }

    pub fn in_memory(data: HashMap<String, String>) -> Self {
        Self {
            path: None,
            data: Mutex::new(data),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.data.lock().unwrap().get(key).cloned()
    }

    pub fn set(&self, key: String, value: String) -> super::Result<()> {
        let mut data = self.data.lock().unwrap();
        data.insert(key, value);
        // Do NOT rollback memory on disk failure. If a token rotation succeeds
        // server-side but the disk write fails, rolling back would leave the
        // in-memory store with the old (now-invalidated) refresh token. The next
        // auto-refresh tick would then try to use that stale token, the server
        // would reject it with a hard auth error, and Supabase SDK would call
        // _removeSession() → SIGNED_OUT. Keeping the new token in memory lets the
        // current session survive; only a cold restart before a successful write
        // would be affected.
        if let Some(path) = &self.path {
            atomic_save(path, &data)?;
        }
        Ok(())
    }

    pub fn remove(&self, key: &str) -> super::Result<()> {
        let mut data = self.data.lock().unwrap();
        let old = data.remove(key);
        if let Some(path) = &self.path {
            if let Err(e) = atomic_save(path, &data) {
                if let Some(prev) = old {
                    data.insert(key.to_string(), prev);
                }
                return Err(e);
            }
        }
        Ok(())
    }

    pub fn clear(&self) -> super::Result<()> {
        let mut data = self.data.lock().unwrap();
        data.clear();
        if let Some(path) = &self.path
            && path.exists()
        {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn replace(&self, data: HashMap<String, String>) {
        *self.data.lock().unwrap() = data;
    }

    pub fn snapshot(&self) -> HashMap<String, String> {
        self.data.lock().unwrap().clone()
    }
}

fn load_data(path: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn atomic_save(path: &Path, data: &HashMap<String, String>) -> super::Result<()> {
    let content = serde_json::to_string(data)?;
    hypr_storage::fs::atomic_write(path, &content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_supports_secure_persistence_adapters() {
        let store = AuthStore::in_memory(HashMap::new());

        store
            .set("session".to_string(), "first".to_string())
            .unwrap();
        assert_eq!(store.get("session").as_deref(), Some("first"));

        store.replace(HashMap::from([(
            "session".to_string(),
            "second".to_string(),
        )]));
        assert_eq!(store.get("session").as_deref(), Some("second"));

        store.remove("session").unwrap();
        assert!(store.snapshot().is_empty());
    }
}
