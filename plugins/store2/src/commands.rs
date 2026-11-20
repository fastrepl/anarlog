use crate::Store2PluginExt;

const NATIVE_SECRET_ACCOUNT_PREFIXES: &[&str] = &["e2ee:"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum SecretCaller {
    Native,
    Renderer,
}

fn validate_secret_coordinate(caller: SecretCaller, scope: &str, key: &str) -> Result<(), String> {
    let account = format!("{scope}:{key}");
    if caller == SecretCaller::Renderer
        && NATIVE_SECRET_ACCOUNT_PREFIXES
            .iter()
            .any(|prefix| account.starts_with(prefix))
    {
        return Err("secure-store account is reserved for native use".to_string());
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn repair_macos_keychain_access() -> Result<(), String> {
    use objc2_security::SecKeychain;

    const ERR_SEC_SUCCESS: i32 = 0;
    const ERR_SEC_USER_CANCELED: i32 = -128;

    #[allow(deprecated)]
    let lock_status = unsafe { SecKeychain::lock(None) };
    if lock_status != ERR_SEC_SUCCESS {
        return Err(format!(
            "macOS couldn't lock your login Keychain (OSStatus {lock_status})."
        ));
    }

    #[allow(deprecated)]
    let unlock_status = unsafe { SecKeychain::unlock(None, 0, std::ptr::null(), false) };
    if unlock_status == ERR_SEC_USER_CANCELED {
        return Err(
            "Keychain unlock was cancelled. Your login Keychain is still locked; run the repair again to unlock it."
                .to_string(),
        );
    }
    if unlock_status != ERR_SEC_SUCCESS {
        return Err(format!(
            "macOS couldn't unlock your login Keychain (OSStatus {unlock_status}). Your login Keychain is still locked."
        ));
    }

    Ok(())
}

fn secure_store<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> anlg_desktop_identity::SecureStore<anlg_desktop_identity::secure_store::KeyringEntry> {
    anlg_desktop_identity::SecureStore::keyring(&app.config().identifier)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn save<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.store2().save().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn repair_keychain_access() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return tauri::async_runtime::spawn_blocking(repair_macos_keychain_access)
            .await
            .map_err(|error| error.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    Err("Keychain repair is only available on macOS.".to_string())
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
    read_secret_for(SecretCaller::Renderer, app, scope, key).await
}

pub async fn read_secret<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<Option<String>, String> {
    read_secret_for(SecretCaller::Native, app, scope, key).await
}

pub fn read_secret_blocking<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<Option<String>, String> {
    read_secret_blocking_for(SecretCaller::Native, app, scope, key)
}

async fn read_secret_for<R: tauri::Runtime>(
    caller: SecretCaller,
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<Option<String>, String> {
    validate_secret_coordinate(caller, &scope, &key)?;
    tauri::async_runtime::spawn_blocking(move || {
        read_secret_blocking_for(caller, &app, &scope, &key)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn read_secret_blocking_for<R: tauri::Runtime>(
    caller: SecretCaller,
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<Option<String>, String> {
    validate_secret_coordinate(caller, scope, key)?;
    secure_store(app).read(scope, key)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_secret<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
    value: String,
) -> Result<(), String> {
    write_secret_for(SecretCaller::Renderer, app, scope, key, value).await
}

pub async fn write_secret<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
    value: String,
) -> Result<(), String> {
    write_secret_for(SecretCaller::Native, app, scope, key, value).await
}

pub fn write_secret_blocking<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
    value: &str,
) -> Result<(), String> {
    write_secret_blocking_for(SecretCaller::Native, app, scope, key, value)
}

async fn write_secret_for<R: tauri::Runtime>(
    caller: SecretCaller,
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
    value: String,
) -> Result<(), String> {
    validate_secret_coordinate(caller, &scope, &key)?;
    tauri::async_runtime::spawn_blocking(move || {
        write_secret_blocking_for(caller, &app, &scope, &key, &value)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn write_secret_blocking_for<R: tauri::Runtime>(
    caller: SecretCaller,
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
    value: &str,
) -> Result<(), String> {
    validate_secret_coordinate(caller, scope, key)?;
    secure_store(app).write(scope, key, value)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_secret<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<(), String> {
    delete_secret_for(SecretCaller::Renderer, app, scope, key).await
}

pub fn delete_secret_blocking<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<(), String> {
    delete_secret_blocking_for(SecretCaller::Native, app, scope, key)
}

async fn delete_secret_for<R: tauri::Runtime>(
    caller: SecretCaller,
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<(), String> {
    validate_secret_coordinate(caller, &scope, &key)?;
    tauri::async_runtime::spawn_blocking(move || {
        delete_secret_blocking_for(caller, &app, &scope, &key)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn delete_secret_blocking_for<R: tauri::Runtime>(
    caller: SecretCaller,
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<(), String> {
    validate_secret_coordinate(caller, scope, key)?;
    secure_store(app).delete(scope, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolates_native_secret_accounts_from_renderer_commands() {
        assert!(validate_secret_coordinate(SecretCaller::Renderer, "provider", "deepgram").is_ok());
        assert!(
            validate_secret_coordinate(
                SecretCaller::Renderer,
                "e2ee",
                "account:user-a:recovery-v1"
            )
            .is_err()
        );
        assert!(
            validate_secret_coordinate(
                SecretCaller::Renderer,
                "e2ee:account",
                "user-a:recovery-v1"
            )
            .is_err()
        );
        assert!(
            validate_secret_coordinate(SecretCaller::Native, "e2ee", "account:user-a:recovery-v1")
                .is_ok()
        );
    }

    #[tokio::test]
    async fn renderer_secret_commands_reject_native_accounts_before_keychain_access() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let app = app.handle().clone();
        let scope = "e2ee".to_string();
        let key = "account:user-a:recovery-v1".to_string();
        let expected = "secure-store account is reserved for native use";

        assert_eq!(
            get_secret(app.clone(), scope.clone(), key.clone())
                .await
                .unwrap_err(),
            expected
        );
        assert_eq!(
            set_secret(
                app.clone(),
                scope.clone(),
                key.clone(),
                "replacement".to_string()
            )
            .await
            .unwrap_err(),
            expected
        );
        assert_eq!(delete_secret(app, scope, key).await.unwrap_err(), expected);
    }
}
