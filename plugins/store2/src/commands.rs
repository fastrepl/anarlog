use crate::Store2PluginExt;

const SECURE_STORE_SUFFIX: &str = "secure-store";
const NATIVE_SECRET_ACCOUNT_PREFIXES: &[&str] = &["e2ee:"];
#[cfg(target_os = "macos")]
const MACOS_KEYCHAIN_ACCESS_ERROR_PREFIX: &str = "macOS couldn't access your login Keychain.";

#[cfg(target_os = "macos")]
const ERR_SEC_AUTH_FAILED: i32 = -25293;

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

fn secure_store_service(identifier: &str) -> String {
    let identifier = match identifier {
        "com.hyprnote.dev" => "com.anarlog.dev",
        "com.hyprnote.staging" => "com.anarlog.staging",
        "com.hyprnote.stable" | "com.hyprnote.Hyprnote" => "com.anarlog.stable",
        identifier => identifier,
    };

    format!("{identifier}.{SECURE_STORE_SUFFIX}")
}

fn secure_store_account(identifier: &str, scope: &str, key: &str) -> String {
    let account = format!("{scope}:{key}");
    if identifier == "com.hyprnote.dev" {
        // Rotate away from dev items whose ACLs captured unstable ad-hoc signatures.
        format!("v2:{account}")
    } else {
        account
    }
}

fn secure_store_error(error: keyring::Error) -> String {
    #[cfg(target_os = "macos")]
    if keychain_error_code(&error) == Some(ERR_SEC_AUTH_FAILED) {
        return format!(
            "{MACOS_KEYCHAIN_ACCESS_ERROR_PREFIX} Use “Repair Keychain Access” below, then try again."
        );
    }

    error.to_string()
}

#[cfg(target_os = "macos")]
fn keychain_error_code(error: &keyring::Error) -> Option<i32> {
    let source = match error {
        keyring::Error::PlatformFailure(source) | keyring::Error::NoStorageAccess(source) => source,
        _ => return None,
    };

    source
        .downcast_ref::<security_framework::base::Error>()
        .map(|error| error.code())
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

fn legacy_secret_locations(identifier: &str, scope: &str, key: &str) -> Vec<(String, String)> {
    let service = secure_store_service(identifier);
    let account = format!("{scope}:{key}");
    let current_account = secure_store_account(identifier, scope, key);
    let legacy_service = format!("{identifier}.{SECURE_STORE_SUFFIX}");
    let mut locations = Vec::new();

    if account != current_account {
        locations.push((service.clone(), account.clone()));
    }
    if legacy_service != service {
        locations.push((legacy_service, account));
    }

    locations
}

fn legacy_secret_entries<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<Vec<keyring::Entry>, String> {
    legacy_secret_locations(&app.config().identifier, scope, key)
        .into_iter()
        .map(|(service, account)| {
            keyring::Entry::new(&service, &account).map_err(|error| error.to_string())
        })
        .collect()
}

fn secret_entry<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    scope: &str,
    key: &str,
) -> Result<keyring::Entry, String> {
    if scope.trim().is_empty() || key.trim().is_empty() {
        return Err("secure-store scope and key must not be empty".to_string());
    }

    let identifier = &app.config().identifier;
    let service = secure_store_service(identifier);
    let account = secure_store_account(identifier, scope, key);
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

async fn read_secret_for<R: tauri::Runtime>(
    caller: SecretCaller,
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<Option<String>, String> {
    validate_secret_coordinate(caller, &scope, &key)?;
    tauri::async_runtime::spawn_blocking(move || {
        let entry = secret_entry(&app, &scope, &key)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => {
                for legacy_entry in legacy_secret_entries(&app, &scope, &key)? {
                    match legacy_entry.get_password() {
                        Ok(secret) => {
                            if entry.set_password(&secret).is_ok() {
                                let _ = legacy_entry.delete_credential();
                            }
                            return Ok(Some(secret));
                        }
                        Err(keyring::Error::NoEntry | keyring::Error::PlatformFailure(_)) => {}
                        Err(error) => return Err(secure_store_error(error)),
                    }
                }
                Ok(None)
            }
            Err(error) => Err(secure_store_error(error)),
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

async fn write_secret_for<R: tauri::Runtime>(
    caller: SecretCaller,
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
    value: String,
) -> Result<(), String> {
    validate_secret_coordinate(caller, &scope, &key)?;
    tauri::async_runtime::spawn_blocking(move || {
        let entry = secret_entry(&app, &scope, &key)?;
        entry.set_password(&value).map_err(secure_store_error)?;
        for legacy_entry in legacy_secret_entries(&app, &scope, &key)? {
            let _ = legacy_entry.delete_credential();
        }
        Ok(())
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
    delete_secret_for(SecretCaller::Renderer, app, scope, key).await
}

async fn delete_secret_for<R: tauri::Runtime>(
    caller: SecretCaller,
    app: tauri::AppHandle<R>,
    scope: String,
    key: String,
) -> Result<(), String> {
    validate_secret_coordinate(caller, &scope, &key)?;
    tauri::async_runtime::spawn_blocking(move || {
        for legacy_entry in legacy_secret_entries(&app, &scope, &key)? {
            match legacy_entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry | keyring::Error::PlatformFailure(_)) => {}
                Err(error) => return Err(secure_store_error(error)),
            }
        }
        let entry = secret_entry(&app, &scope, &key)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(secure_store_error(error)),
        }
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_anarlog_service_names_for_legacy_bundle_identifiers() {
        assert_eq!(
            secure_store_service("com.hyprnote.dev"),
            "com.anarlog.dev.secure-store"
        );
        assert_eq!(
            secure_store_service("com.hyprnote.staging"),
            "com.anarlog.staging.secure-store"
        );
        assert_eq!(
            secure_store_service("com.hyprnote.stable"),
            "com.anarlog.stable.secure-store"
        );
        assert_eq!(
            secure_store_service("com.hyprnote.Hyprnote"),
            "com.anarlog.stable.secure-store"
        );
    }

    #[test]
    fn preserves_unknown_service_identifiers() {
        assert_eq!(
            secure_store_service("com.example.app"),
            "com.example.app.secure-store"
        );
    }

    #[test]
    fn versions_dev_accounts_across_signing_changes() {
        assert_eq!(
            secure_store_account("com.hyprnote.dev", "provider", "deepgram"),
            "v2:provider:deepgram"
        );
        assert_eq!(
            secure_store_account("com.hyprnote.stable", "provider", "deepgram"),
            "provider:deepgram"
        );
    }

    #[test]
    fn migrates_all_previous_dev_secret_locations() {
        assert_eq!(
            legacy_secret_locations("com.hyprnote.dev", "provider", "deepgram"),
            vec![
                (
                    "com.anarlog.dev.secure-store".to_string(),
                    "provider:deepgram".to_string(),
                ),
                (
                    "com.hyprnote.dev.secure-store".to_string(),
                    "provider:deepgram".to_string(),
                ),
            ]
        );
    }

    #[test]
    fn skips_duplicate_legacy_secret_locations() {
        assert!(legacy_secret_locations("com.example.app", "provider", "deepgram").is_empty());
    }

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

    #[cfg(target_os = "macos")]
    #[test]
    fn explains_macos_keychain_access_failures() {
        let error = keyring::Error::PlatformFailure(Box::new(
            security_framework::base::Error::from_code(ERR_SEC_AUTH_FAILED),
        ));

        assert_eq!(
            secure_store_error(error),
            "macOS couldn't access your login Keychain. Use “Repair Keychain Access” below, then try again."
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn preserves_unrelated_macos_keychain_failures() {
        let platform_error = security_framework::base::Error::from_code(-34018);
        let expected = format!("Platform failure: {platform_error}");
        let error = keyring::Error::PlatformFailure(Box::new(platform_error));

        assert_eq!(secure_store_error(error), expected);
    }
}
