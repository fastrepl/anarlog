//! API keys live in the OS credential store, addressed exactly like
//! `plugins/store2` (`secure_store_service` / `secure_store_account`) so both
//! apps read and write the same entries.

const SECURE_STORE_SUFFIX: &str = "secure-store";
/// `PROVIDER_SECRET_SCOPE` in `apps/desktop/src/settings/providers.ts`.
pub const PROVIDER_SECRET_SCOPE: &str = "ai-provider-api-keys";

#[cfg(target_os = "linux")]
const LINUX_SECRET_SERVICE_ACCESS_ERROR: &str =
    "Linux couldn't access Secret Service. Unlock your login keyring, then try again.";
#[cfg(target_os = "linux")]
const LINUX_SECRET_SERVICE_UNAVAILABLE_ERROR: &str =
    "Linux Secret Service is unavailable. Start your desktop keyring service, then try again.";

fn service(app_id: &str) -> String {
    let identifier = match app_id {
        "com.hyprnote.dev" => "com.anarlog.dev",
        "com.hyprnote.staging" => "com.anarlog.staging",
        "com.hyprnote.stable" | "com.hyprnote.Hyprnote" => "com.anarlog.stable",
        identifier => identifier,
    };
    format!("{identifier}.{SECURE_STORE_SUFFIX}")
}

fn account(app_id: &str, scope: &str, key: &str) -> String {
    let account = format!("{scope}:{key}");
    if app_id == "com.hyprnote.dev" {
        // Rotate away from dev items whose ACLs captured unstable ad-hoc signatures.
        format!("v2:{account}")
    } else {
        account
    }
}

/// `legacy_secret_locations`
fn legacy_locations(app_id: &str, scope: &str, key: &str) -> Vec<(String, String)> {
    let service = service(app_id);
    let account_plain = format!("{scope}:{key}");
    let current = account(app_id, scope, key);
    let legacy_service = format!("{app_id}.{SECURE_STORE_SUFFIX}");
    let mut locations = Vec::new();
    if account_plain != current {
        locations.push((service.clone(), account_plain.clone()));
    }
    if legacy_service != service {
        locations.push((legacy_service, account_plain));
    }
    locations
}

fn describe(error: keyring::Error) -> String {
    #[cfg(target_os = "linux")]
    match error {
        keyring::Error::NoStorageAccess(_) => {
            return LINUX_SECRET_SERVICE_ACCESS_ERROR.to_string();
        }
        keyring::Error::PlatformFailure(_) => {
            return LINUX_SECRET_SERVICE_UNAVAILABLE_ERROR.to_string();
        }
        _ => {}
    }
    error.to_string()
}

fn entry(app_id: &str, scope: &str, key: &str) -> Result<keyring::Entry, String> {
    if scope.trim().is_empty() || key.trim().is_empty() {
        return Err("secure-store scope and key must not be empty".to_string());
    }
    keyring::Entry::new(&service(app_id), &account(app_id, scope, key)).map_err(describe)
}

/// `read_secret_blocking`: the current entry, falling back to (and migrating
/// from) the legacy locations.
pub fn read(app_id: &str, scope: &str, key: &str) -> Result<Option<String>, String> {
    let entry = entry(app_id, scope, key)?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => {
            for (service, account) in legacy_locations(app_id, scope, key) {
                let legacy = keyring::Entry::new(&service, &account).map_err(describe)?;
                match legacy.get_password() {
                    Ok(secret) => {
                        if entry.set_password(&secret).is_ok() {
                            let _ = legacy.delete_credential();
                        }
                        return Ok(Some(secret));
                    }
                    Err(keyring::Error::NoEntry | keyring::Error::PlatformFailure(_)) => {}
                    Err(error) => return Err(describe(error)),
                }
            }
            Ok(None)
        }
        Err(error) => Err(describe(error)),
    }
}

/// `write_secret_blocking` / `delete_secret_blocking`: an empty value deletes.
pub fn write(app_id: &str, scope: &str, key: &str, value: &str) -> Result<(), String> {
    let entry = entry(app_id, scope, key)?;
    if value.is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(describe(error)),
        };
    }
    entry.set_password(value).map_err(describe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_entries_use_the_anarlog_service_and_v2_accounts() {
        assert_eq!(service("com.hyprnote.dev"), "com.anarlog.dev.secure-store");
        assert_eq!(
            service("com.hyprnote.stable"),
            "com.anarlog.stable.secure-store"
        );
        assert_eq!(
            account("com.hyprnote.dev", PROVIDER_SECRET_SCOPE, "llm:openai"),
            "v2:ai-provider-api-keys:llm:openai"
        );
        assert_eq!(
            account("com.hyprnote.stable", PROVIDER_SECRET_SCOPE, "llm:openai"),
            "ai-provider-api-keys:llm:openai"
        );
        assert_eq!(
            legacy_locations("com.hyprnote.dev", PROVIDER_SECRET_SCOPE, "stt:deepgram"),
            vec![
                (
                    "com.anarlog.dev.secure-store".to_string(),
                    "ai-provider-api-keys:stt:deepgram".to_string()
                ),
                (
                    "com.hyprnote.dev.secure-store".to_string(),
                    "ai-provider-api-keys:stt:deepgram".to_string()
                ),
            ]
        );
    }
}
