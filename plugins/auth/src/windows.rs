use std::collections::HashMap;
use std::path::Path;

#[cfg(target_os = "windows")]
pub(crate) fn load_auth<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::Result<HashMap<String, String>> {
    let plaintext_path = crate::migrate::auth_path(app)?;
    let secure_path = crate::migrate::windows_secure_auth_path(app)?;

    if secure_path.is_file() {
        let auth = match anlg_storage::windows_auth::load(&secure_path) {
            Ok(auth) => auth,
            Err(error) => {
                tracing::warn!(%error, "ignoring_unreadable_windows_auth");
                HashMap::new()
            }
        };
        crate::migrate::discard_windows_plaintext_auth(app);
        return Ok(auth);
    }

    if !plaintext_path.is_file() {
        return Ok(HashMap::new());
    }

    let auth = match read_plaintext_auth(&plaintext_path) {
        Ok(auth) => auth,
        Err(error) => {
            tracing::warn!(%error, "ignoring_unreadable_plaintext_auth");
            crate::migrate::discard_windows_plaintext_auth(app);
            return Ok(HashMap::new());
        }
    };

    if let Err(error) = anlg_storage::windows_auth::persist(&secure_path, &auth) {
        tracing::warn!(%error, "failed_to_migrate_auth_to_windows_data_protection");
        return Ok(auth);
    }

    crate::migrate::discard_windows_plaintext_auth(app);
    Ok(auth)
}

#[cfg(target_os = "windows")]
pub(crate) fn persist_auth<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    auth: &HashMap<String, String>,
) -> crate::Result<()> {
    let secure_path = crate::migrate::windows_secure_auth_path(app)?;
    anlg_storage::windows_auth::persist(&secure_path, auth)
        .map_err(|error| crate::Error::Storage(error.to_string()))?;
    crate::migrate::discard_windows_plaintext_auth(app);
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn clear_auth<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> crate::Result<()> {
    let secure_path = crate::migrate::windows_secure_auth_path(app)?;
    crate::migrate::discard_windows_plaintext_auth(app);
    anlg_storage::windows_auth::clear(&secure_path)
        .map_err(|error| crate::Error::Storage(error.to_string()))
}

fn read_plaintext_auth(path: &Path) -> crate::Result<HashMap<String, String>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}
