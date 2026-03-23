use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// In-memory state is the source of truth, not the file.
/// This lets the session survive disk write failures.
pub struct AuthStore {
    path: PathBuf,
    data: Mutex<HashMap<String, String>>,
}

impl AuthStore {
    pub fn load(path: PathBuf) -> Self {
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            path,
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
        atomic_save(&self.path, &data)?;
        Ok(())
    }

    pub fn remove(&self, key: &str) -> super::Result<()> {
        let mut data = self.data.lock().unwrap();
        let old = data.remove(key);
        if let Err(e) = atomic_save(&self.path, &data) {
            if let Some(prev) = old {
                data.insert(key.to_string(), prev);
            }
            return Err(e);
        }
        Ok(())
    }

    pub fn clear(&self) -> super::Result<()> {
        let mut data = self.data.lock().unwrap();
        data.clear();
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    pub fn snapshot(&self) -> HashMap<String, String> {
        self.data.lock().unwrap().clone()
    }
}

fn atomic_save(path: &Path, data: &HashMap<String, String>) -> super::Result<()> {
    let content = serde_json::to_string(data)?;
    hypr_storage::fs::atomic_write(path, &content)?;
    Ok(())
}
