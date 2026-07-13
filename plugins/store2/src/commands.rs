use crate::Store2PluginExt;

const SECURE_STORE_SUFFIX: &str = "secure-store";

fn secret_entry<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<keyring::Entry, String> {
    if scope.trim().is_empty() || key.trim().is_empty() {
        return Err("secure-store scope and key must not be empty".to_string());
    }

    let service = format!("{}.{}", app.config().identifier, SECURE_STORE_SUFFIX);
    let account = format!("{scope}:{key}");
    keyring::Entry::new(&service, &account).map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn save<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.store2()
        .store()
        .map_err(|e| e.to_string())?
        .save()
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_str<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<Option<String>, String> {
    let store = app
        .store2()
        .scoped_store::<String>(scope)
        .map_err(|e| e.to_string())?;

    store.get::<String>(key).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_str<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
    value: String,
) -> Result<(), String> {
    let store = app
        .store2()
        .scoped_store::<String>(scope)
        .map_err(|e| e.to_string())?;

    store.set(key, value).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_bool<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<Option<bool>, String> {
    let store = app
        .store2()
        .scoped_store::<String>(scope)
        .map_err(|e| e.to_string())?;

    store.get::<bool>(key).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_bool<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
    value: bool,
) -> Result<(), String> {
    let store = app
        .store2()
        .scoped_store::<String>(scope)
        .map_err(|e| e.to_string())?;

    store.set(key, value).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_number<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<Option<f64>, String> {
    let store = app
        .store2()
        .scoped_store::<String>(scope)
        .map_err(|e| e.to_string())?;

    store.get::<f64>(key).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_number<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
    value: f64,
) -> Result<(), String> {
    let store = app
        .store2()
        .scoped_store::<String>(scope)
        .map_err(|e| e.to_string())?;

    store.set(key, value).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_secret<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = secret_entry(&app, &scope, &key)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_secret<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
    value: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = secret_entry(&app, &scope, &key)?;
        entry
            .set_password(&value)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_secret<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = secret_entry(&app, &scope, &key)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    })
    .await
    .map_err(|error| error.to_string())?
}
