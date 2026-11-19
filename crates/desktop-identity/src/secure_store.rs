use crate::chunked::{self, ChunkedError};
pub use crate::chunked::SecretSlot;

const SECURE_STORE_SUFFIX: &str = "secure-store";
#[cfg(target_os = "macos")]
const MACOS_KEYCHAIN_ACCESS_ERROR_PREFIX: &str = "macOS couldn't access your login Keychain.";
#[cfg(target_os = "linux")]
const LINUX_SECRET_SERVICE_ACCESS_ERROR: &str =
    "Linux couldn't access Secret Service. Unlock your login keyring, then try again.";
#[cfg(target_os = "linux")]
const LINUX_SECRET_SERVICE_UNAVAILABLE_ERROR: &str =
    "Linux Secret Service is unavailable. Start your desktop keyring service, then try again.";

#[cfg(target_os = "macos")]
const ERR_SEC_AUTH_FAILED: i32 = -25293;

/// The keychain service (Windows: target suffix) holding a desktop channel's secrets.
pub fn service(identifier: &str) -> String {
    let identifier = match identifier {
        "com.hyprnote.dev" => "com.anarlog.dev",
        "com.hyprnote.staging" => "com.anarlog.staging",
        "com.hyprnote.stable" | "com.hyprnote.Hyprnote" => "com.anarlog.stable",
        identifier => identifier,
    };

    format!("{identifier}.{SECURE_STORE_SUFFIX}")
}

/// The keychain account holding `scope`'s `key`.
pub fn account(identifier: &str, scope: &str, key: &str) -> String {
    let account = format!("{scope}:{key}");
    if identifier == "com.hyprnote.dev" {
        // Rotate away from dev items whose ACLs captured unstable ad-hoc signatures.
        format!("v2:{account}")
    } else {
        account
    }
}

/// Earlier `(service, account)` locations of a secret; reads move a secret found there.
pub fn legacy_locations(identifier: &str, scope: &str, key: &str) -> Vec<(String, String)> {
    let service = service(identifier);
    let legacy_account = format!("{scope}:{key}");
    let current_account = account(identifier, scope, key);
    let legacy_service = format!("{identifier}.{SECURE_STORE_SUFFIX}");
    let mut locations = Vec::new();

    if legacy_account != current_account {
        locations.push((service.clone(), legacy_account.clone()));
    }
    if legacy_service != service {
        locations.push((legacy_service, legacy_account));
    }

    locations
}

/// The message the desktop app shows for a credential-store failure.
pub fn error_message(error: keyring::Error) -> String {
    #[cfg(target_os = "macos")]
    if keychain_error_code(&error) == Some(ERR_SEC_AUTH_FAILED) {
        return format!(
            "{MACOS_KEYCHAIN_ACCESS_ERROR_PREFIX} Use “Repair Keychain Access” below, then try again."
        );
    }

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

fn chunked_error(error: ChunkedError) -> String {
    match error {
        ChunkedError::Keyring(error) => error_message(error),
        ChunkedError::Slot(error) => error,
        ChunkedError::Incomplete => {
            "secure-store secret is incomplete; save it again to repair it".to_string()
        }
    }
}

pub type KeyringEntry = fn(&str, &str) -> keyring::Result<keyring::Entry>;

/// A desktop channel's secrets: shipping's service and account names, chunking for values
/// over the platform's size limit, and migration out of legacy locations. Calls block on the
/// platform credential store, which may wait for a user prompt.
pub struct SecureStore<F> {
    identifier: String,
    entry: F,
}

impl SecureStore<KeyringEntry> {
    pub fn keyring(identifier: &str) -> Self {
        Self::new(identifier, keyring::Entry::new)
    }
}

impl<F, S> SecureStore<F>
where
    F: Fn(&str, &str) -> keyring::Result<S>,
    S: SecretSlot,
{
    /// `entry` opens the credential at `(service, account)`.
    pub fn new(identifier: &str, entry: F) -> Self {
        Self {
            identifier: identifier.to_string(),
            entry,
        }
    }

    pub fn read(&self, scope: &str, key: &str) -> Result<Option<String>, String> {
        let location = self.location(scope, key)?;
        match location.read() {
            Ok(secret) => Ok(Some(secret)),
            Err(ChunkedError::Keyring(keyring::Error::NoEntry)) => {
                for legacy_entry in self.legacy_entries(scope, key)? {
                    match legacy_entry.get() {
                        Ok(secret) => {
                            if location.write(&secret).is_ok() {
                                let _ = legacy_entry.delete();
                            }
                            return Ok(Some(secret));
                        }
                        Err(keyring::Error::NoEntry | keyring::Error::PlatformFailure(_)) => {}
                        Err(error) => return Err(error_message(error)),
                    }
                }
                Ok(None)
            }
            Err(error) => Err(chunked_error(error)),
        }
    }

    pub fn write(&self, scope: &str, key: &str, value: &str) -> Result<(), String> {
        self.location(scope, key)?.write(value)?;
        for legacy_entry in self.legacy_entries(scope, key)? {
            let _ = legacy_entry.delete();
        }
        Ok(())
    }

    pub fn delete(&self, scope: &str, key: &str) -> Result<(), String> {
        for legacy_entry in self.legacy_entries(scope, key)? {
            match legacy_entry.delete() {
                Ok(()) | Err(keyring::Error::NoEntry | keyring::Error::PlatformFailure(_)) => {}
                Err(error) => return Err(error_message(error)),
            }
        }
        self.location(scope, key)?.delete()
    }

    fn location(&self, scope: &str, key: &str) -> Result<Location<'_, F>, String> {
        if scope.trim().is_empty() || key.trim().is_empty() {
            return Err("secure-store scope and key must not be empty".to_string());
        }

        Ok(Location {
            service: service(&self.identifier),
            account: account(&self.identifier, scope, key),
            entry: &self.entry,
        })
    }

    fn legacy_entries(&self, scope: &str, key: &str) -> Result<Vec<S>, String> {
        legacy_locations(&self.identifier, scope, key)
            .into_iter()
            .map(|(service, account)| (self.entry)(&service, &account).map_err(error_message))
            .collect()
    }
}

struct Location<'a, F> {
    service: String,
    account: String,
    entry: &'a F,
}

impl<F, S> Location<'_, F>
where
    F: Fn(&str, &str) -> keyring::Result<S>,
    S: SecretSlot,
{
    fn slot(&self) -> impl Fn(&str) -> Result<S, String> + '_ {
        |account| (self.entry)(&self.service, account).map_err(error_message)
    }

    fn read(&self) -> Result<String, ChunkedError> {
        chunked::read(self.slot(), &self.account)
    }

    fn write(&self, value: &str) -> Result<(), String> {
        chunked::write(self.slot(), &self.account, value).map_err(chunked_error)
    }

    fn delete(&self) -> Result<(), String> {
        chunked::delete(self.slot(), &self.account).map_err(chunked_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    #[test]
    fn maps_bundle_identifiers_to_secure_store_services() {
        for (identifier, expected) in [
            ("com.hyprnote.dev", "com.anarlog.dev.secure-store"),
            ("com.hyprnote.staging", "com.anarlog.staging.secure-store"),
            ("com.hyprnote.stable", "com.anarlog.stable.secure-store"),
            ("com.hyprnote.Hyprnote", "com.anarlog.stable.secure-store"),
            ("com.example.app", "com.example.app.secure-store"),
        ] {
            assert_eq!(
                service(identifier),
                expected,
                "unexpected service for {identifier}"
            );
        }
    }

    #[test]
    fn versions_dev_accounts_across_signing_changes() {
        assert_eq!(
            account("com.hyprnote.dev", "provider", "deepgram"),
            "v2:provider:deepgram"
        );
        assert_eq!(
            account("com.hyprnote.stable", "provider", "deepgram"),
            "provider:deepgram"
        );
    }

    #[test]
    fn lists_legacy_secret_locations_without_duplicates() {
        for (identifier, expected) in [
            (
                "com.hyprnote.dev",
                vec![
                    (
                        "com.anarlog.dev.secure-store".to_string(),
                        "provider:deepgram".to_string(),
                    ),
                    (
                        "com.hyprnote.dev.secure-store".to_string(),
                        "provider:deepgram".to_string(),
                    ),
                ],
            ),
            ("com.example.app", vec![]),
        ] {
            let locations = legacy_locations(identifier, "provider", "deepgram");
            assert_eq!(
                locations, expected,
                "unexpected legacy locations for {identifier}"
            );
            let unique: std::collections::HashSet<_> = locations.iter().collect();
            assert_eq!(
                locations.len(),
                unique.len(),
                "duplicate locations for {identifier}"
            );
        }
    }

    /// Every identifier the shipping app has used: its service, an account and the legacy
    /// locations a read migrates from.
    #[test]
    fn identifier_keychain_golden() {
        for (identifier, expected_service, expected_account, expected_legacy) in [
            (
                "com.hyprnote.dev",
                "com.anarlog.dev.secure-store",
                "v2:ai-provider-api-keys:llm:openai",
                &[
                    (
                        "com.anarlog.dev.secure-store",
                        "ai-provider-api-keys:llm:openai",
                    ),
                    (
                        "com.hyprnote.dev.secure-store",
                        "ai-provider-api-keys:llm:openai",
                    ),
                ][..],
            ),
            (
                "com.hyprnote.staging",
                "com.anarlog.staging.secure-store",
                "ai-provider-api-keys:llm:openai",
                &[(
                    "com.hyprnote.staging.secure-store",
                    "ai-provider-api-keys:llm:openai",
                )][..],
            ),
            (
                "com.hyprnote.nightly",
                "com.hyprnote.nightly.secure-store",
                "ai-provider-api-keys:llm:openai",
                &[][..],
            ),
            (
                "com.hyprnote.stable",
                "com.anarlog.stable.secure-store",
                "ai-provider-api-keys:llm:openai",
                &[(
                    "com.hyprnote.stable.secure-store",
                    "ai-provider-api-keys:llm:openai",
                )][..],
            ),
            (
                "com.hyprnote.Hyprnote",
                "com.anarlog.stable.secure-store",
                "ai-provider-api-keys:llm:openai",
                &[(
                    "com.hyprnote.Hyprnote.secure-store",
                    "ai-provider-api-keys:llm:openai",
                )][..],
            ),
            (
                "com.hyprnote.desktop",
                "com.hyprnote.desktop.secure-store",
                "ai-provider-api-keys:llm:openai",
                &[][..],
            ),
            (
                "so.anarlog.Anarlog",
                "so.anarlog.Anarlog.secure-store",
                "ai-provider-api-keys:llm:openai",
                &[][..],
            ),
        ] {
            assert_eq!(service(identifier), expected_service, "{identifier}");
            assert_eq!(
                account(identifier, "ai-provider-api-keys", "llm:openai"),
                expected_account,
                "{identifier}"
            );
            let expected_legacy: Vec<(String, String)> = expected_legacy
                .iter()
                .map(|(service, account)| (service.to_string(), account.to_string()))
                .collect();
            assert_eq!(
                legacy_locations(identifier, "ai-provider-api-keys", "llm:openai"),
                expected_legacy,
                "{identifier}"
            );
        }
    }

    const WINDOWS_MAX_UTF16_UNITS: usize = 1280;

    #[derive(Default)]
    struct Keychain {
        items: RefCell<BTreeMap<(String, String), String>>,
    }

    struct Item {
        keychain: Rc<Keychain>,
        key: (String, String),
    }

    impl SecretSlot for Item {
        fn get(&self) -> keyring::Result<String> {
            let items = self.keychain.items.borrow();
            items.get(&self.key).cloned().ok_or(keyring::Error::NoEntry)
        }

        fn set(&self, value: &str) -> keyring::Result<()> {
            if value.encode_utf16().count() > WINDOWS_MAX_UTF16_UNITS {
                return Err(keyring::Error::TooLong("password".to_string(), 2560));
            }
            let mut items = self.keychain.items.borrow_mut();
            items.insert(self.key.clone(), value.to_string());
            Ok(())
        }

        fn delete(&self) -> keyring::Result<()> {
            let mut items = self.keychain.items.borrow_mut();
            items
                .remove(&self.key)
                .map(|_| ())
                .ok_or(keyring::Error::NoEntry)
        }
    }

    fn store(
        identifier: &str,
        keychain: &Rc<Keychain>,
    ) -> SecureStore<impl Fn(&str, &str) -> keyring::Result<Item>> {
        let keychain = keychain.clone();
        SecureStore::new(identifier, move |service: &str, account: &str| {
            Ok(Item {
                keychain: keychain.clone(),
                key: (service.to_string(), account.to_string()),
            })
        })
    }

    fn item(service: &str, account: &str) -> (String, String) {
        (service.to_string(), account.to_string())
    }

    #[test]
    fn reads_move_a_legacy_secret_to_the_current_location() {
        let keychain = Rc::new(Keychain::default());
        keychain.items.borrow_mut().insert(
            item("com.hyprnote.stable.secure-store", "e2ee:account:a:recovery-v1"),
            "code".to_string(),
        );
        let store = store("com.hyprnote.stable", &keychain);

        assert_eq!(
            store.read("e2ee", "account:a:recovery-v1").unwrap(),
            Some("code".to_string())
        );
        assert_eq!(
            keychain.items.borrow().keys().cloned().collect::<Vec<_>>(),
            vec![item(
                "com.anarlog.stable.secure-store",
                "e2ee:account:a:recovery-v1"
            )]
        );
        assert_eq!(store.read("e2ee", "account:b:recovery-v1").unwrap(), None);
    }

    #[test]
    fn writes_and_deletes_clear_legacy_locations() {
        let keychain = Rc::new(Keychain::default());
        for (service, account) in [
            ("com.anarlog.dev.secure-store", "provider:deepgram"),
            ("com.hyprnote.dev.secure-store", "provider:deepgram"),
        ] {
            let mut items = keychain.items.borrow_mut();
            items.insert(item(service, account), "old".to_string());
        }
        let store = store("com.hyprnote.dev", &keychain);

        store.write("provider", "deepgram", "new").unwrap();
        assert_eq!(
            keychain.items.borrow().clone(),
            BTreeMap::from([(
                item("com.anarlog.dev.secure-store", "v2:provider:deepgram"),
                "new".to_string()
            )])
        );

        store.delete("provider", "deepgram").unwrap();
        assert!(keychain.items.borrow().is_empty());
        assert!(store.read(" ", "deepgram").is_err());
    }

    /// The chunk layout shipping writes when a value exceeds the platform limit.
    #[test]
    fn chunk_layout_golden() {
        let keychain = Rc::new(Keychain::default());
        let store = store("com.hyprnote.stable", &keychain);
        let secret = "x".repeat(3000);
        store
            .write("ai-provider-api-keys", "llm:openai", &secret)
            .unwrap();

        let items = keychain.items.borrow().clone();
        let service = "com.anarlog.stable.secure-store";
        let primary = &items[&item(service, "ai-provider-api-keys:llm:openai")];
        let manifest = primary
            .strip_prefix("anarlog-secure-store-chunks:v1:")
            .unwrap();
        let (generation, count) = manifest.split_once(':').unwrap();
        assert_eq!(count, "3");
        for (index, length) in [1024, 1024, 952].into_iter().enumerate() {
            let chunk = format!("ai-provider-api-keys:llm:openai:chunk:{generation}:{index}");
            assert_eq!(items[&item(service, &chunk)].len(), length, "{chunk}");
        }
        assert_eq!(items.len(), 4);
        assert_eq!(
            store.read("ai-provider-api-keys", "llm:openai").unwrap(),
            Some(secret)
        );
    }
}
