use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

// Windows Credential Manager caps a credential blob at 2560 bytes, i.e. 1280
// UTF-16 code units. Secrets that exceed a platform's limit are split across
// chunk entries, and the primary entry holds a manifest pointing at them.
const MANIFEST_PREFIX: &str = "anarlog-secure-store-chunks:v1:";
const CHUNK_UTF16_UNITS: usize = 1024;

pub(crate) trait SecretSlot {
    fn get(&self) -> keyring::Result<String>;
    fn set(&self, value: &str) -> keyring::Result<()>;
    fn delete(&self) -> keyring::Result<()>;
}

impl SecretSlot for keyring::Entry {
    fn get(&self) -> keyring::Result<String> {
        self.get_password()
    }

    fn set(&self, value: &str) -> keyring::Result<()> {
        self.set_password(value)
    }

    fn delete(&self) -> keyring::Result<()> {
        self.delete_credential()
    }
}

#[derive(Debug)]
pub(crate) enum ChunkedError {
    Keyring(keyring::Error),
    Slot(String),
    Incomplete,
}

impl From<keyring::Error> for ChunkedError {
    fn from(error: keyring::Error) -> Self {
        Self::Keyring(error)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Manifest {
    generation: String,
    count: usize,
}

impl Manifest {
    fn parse(value: &str) -> Option<Self> {
        let rest = value.strip_prefix(MANIFEST_PREFIX)?;
        let (generation, count) = rest.split_once(':')?;
        if generation.is_empty() || !generation.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        let count = count.parse().ok().filter(|count| *count > 0)?;
        Some(Self {
            generation: generation.to_string(),
            count,
        })
    }

    fn encode(&self) -> String {
        format!("{MANIFEST_PREFIX}{}:{}", self.generation, self.count)
    }

    fn chunk_accounts(&self, account: &str) -> impl Iterator<Item = String> + '_ {
        let account = account.to_string();
        (0..self.count).map(move |index| format!("{account}:chunk:{}:{index}", self.generation))
    }
}

fn split_utf16_chunks(value: &str, max_units: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut units = 0;
    for (index, ch) in value.char_indices() {
        let len = ch.len_utf16();
        if units + len > max_units {
            chunks.push(&value[start..index]);
            start = index;
            units = 0;
        }
        units += len;
    }
    if start < value.len() {
        chunks.push(&value[start..]);
    }
    chunks
}

fn new_generation() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}")
}

fn delete_chunks<S: SecretSlot>(
    slot: &impl Fn(&str) -> Result<S, String>,
    account: &str,
    manifest: &Manifest,
) {
    for chunk_account in manifest.chunk_accounts(account) {
        if let Ok(chunk) = slot(&chunk_account) {
            let _ = chunk.delete();
        }
    }
}

fn existing_manifest<S: SecretSlot>(primary: &S) -> Option<Manifest> {
    primary.get().ok().and_then(|value| Manifest::parse(&value))
}

static ACCOUNT_LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(Default::default);

fn account_lock(account: &str) -> Arc<Mutex<()>> {
    ACCOUNT_LOCKS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(account.to_string())
        .or_default()
        .clone()
}

pub(crate) fn read<S: SecretSlot>(
    slot: impl Fn(&str) -> Result<S, String>,
    account: &str,
) -> Result<String, ChunkedError> {
    let lock = account_lock(account);
    let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let value = slot(account).map_err(ChunkedError::Slot)?.get()?;
    let Some(manifest) = Manifest::parse(&value) else {
        return Ok(value);
    };

    let mut secret = String::new();
    for chunk_account in manifest.chunk_accounts(account) {
        match slot(&chunk_account).map_err(ChunkedError::Slot)?.get() {
            Ok(chunk) => secret.push_str(&chunk),
            Err(keyring::Error::NoEntry) => return Err(ChunkedError::Incomplete),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(secret)
}

pub(crate) fn write<S: SecretSlot>(
    slot: impl Fn(&str) -> Result<S, String>,
    account: &str,
    value: &str,
) -> Result<(), ChunkedError> {
    let lock = account_lock(account);
    let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let primary = slot(account).map_err(ChunkedError::Slot)?;
    let previous = existing_manifest(&primary);

    match primary.set(value) {
        Ok(()) => {}
        Err(keyring::Error::TooLong(..)) => {
            let chunks = split_utf16_chunks(value, CHUNK_UTF16_UNITS);
            let manifest = Manifest {
                generation: new_generation(),
                count: chunks.len(),
            };
            let written = manifest
                .chunk_accounts(account)
                .zip(chunks)
                .try_for_each(|(chunk_account, chunk)| {
                    slot(&chunk_account)
                        .map_err(ChunkedError::Slot)?
                        .set(chunk)
                        .map_err(ChunkedError::from)
                })
                .and_then(|()| primary.set(&manifest.encode()).map_err(ChunkedError::from));
            if let Err(error) = written {
                delete_chunks(&slot, account, &manifest);
                return Err(error);
            }
        }
        Err(error) => return Err(error.into()),
    }

    if let Some(previous) = previous {
        delete_chunks(&slot, account, &previous);
    }
    Ok(())
}

pub(crate) fn delete<S: SecretSlot>(
    slot: impl Fn(&str) -> Result<S, String>,
    account: &str,
) -> Result<(), ChunkedError> {
    let lock = account_lock(account);
    let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let primary = slot(account).map_err(ChunkedError::Slot)?;
    let manifest = existing_manifest(&primary);
    match primary.delete() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(error) => return Err(error.into()),
    }
    if let Some(manifest) = manifest {
        delete_chunks(&slot, account, &manifest);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    const WINDOWS_MAX_UTF16_UNITS: usize = 1280;

    #[derive(Default)]
    struct Store {
        entries: RefCell<HashMap<String, String>>,
        fail_account: RefCell<Option<String>>,
    }

    struct Slot {
        store: Rc<Store>,
        account: String,
    }

    impl SecretSlot for Slot {
        fn get(&self) -> keyring::Result<String> {
            self.store
                .entries
                .borrow()
                .get(&self.account)
                .cloned()
                .ok_or(keyring::Error::NoEntry)
        }

        fn set(&self, value: &str) -> keyring::Result<()> {
            if value.encode_utf16().count() > WINDOWS_MAX_UTF16_UNITS {
                return Err(keyring::Error::TooLong(
                    "password encoded as UTF-16".to_string(),
                    2560,
                ));
            }
            if self.store.fail_account.borrow().as_deref() == Some(self.account.as_str()) {
                return Err(keyring::Error::Invalid(
                    "mock".to_string(),
                    "failure".to_string(),
                ));
            }
            self.store
                .entries
                .borrow_mut()
                .insert(self.account.clone(), value.to_string());
            Ok(())
        }

        fn delete(&self) -> keyring::Result<()> {
            if self.store.fail_account.borrow().as_deref() == Some(self.account.as_str()) {
                return Err(keyring::Error::Invalid(
                    "mock".to_string(),
                    "failure".to_string(),
                ));
            }
            self.store
                .entries
                .borrow_mut()
                .remove(&self.account)
                .map(|_| ())
                .ok_or(keyring::Error::NoEntry)
        }
    }

    fn slots(store: &Rc<Store>) -> impl Fn(&str) -> Result<Slot, String> + '_ {
        move |account| {
            Ok(Slot {
                store: store.clone(),
                account: account.to_string(),
            })
        }
    }

    fn chatgpt_sized_credential() -> String {
        format!(
            r#"{{"type":"oauth","refresh":"{}","access":"{}","expires":1790600950000,"accountId":"acct"}}"#,
            "r".repeat(400),
            "eyJ".repeat(900),
        )
    }

    #[test]
    fn round_trips_secrets_longer_than_the_platform_limit() {
        let store = Rc::new(Store::default());
        let secret = chatgpt_sized_credential();
        write(slots(&store), "provider:openai", &secret).unwrap();

        assert!(store.entries.borrow().len() > 1);
        assert!(
            store
                .entries
                .borrow()
                .values()
                .all(|value| value.encode_utf16().count() <= WINDOWS_MAX_UTF16_UNITS)
        );
        assert_eq!(read(slots(&store), "provider:openai").unwrap(), secret);
    }

    #[test]
    fn replacing_a_chunked_secret_removes_stale_chunks() {
        let store = Rc::new(Store::default());
        write(
            slots(&store),
            "provider:openai",
            &chatgpt_sized_credential(),
        )
        .unwrap();
        write(slots(&store), "provider:openai", "sk-short").unwrap();

        assert_eq!(store.entries.borrow().len(), 1);
        assert_eq!(read(slots(&store), "provider:openai").unwrap(), "sk-short");

        let longer = chatgpt_sized_credential().repeat(2);
        write(slots(&store), "provider:openai", &longer).unwrap();
        let chunk_count = store.entries.borrow().len() - 1;
        write(
            slots(&store),
            "provider:openai",
            &chatgpt_sized_credential(),
        )
        .unwrap();

        assert!(store.entries.borrow().len() - 1 < chunk_count);
        assert_eq!(
            read(slots(&store), "provider:openai").unwrap(),
            chatgpt_sized_credential()
        );
    }

    #[test]
    fn delete_removes_chunks() {
        let store = Rc::new(Store::default());
        write(
            slots(&store),
            "provider:openai",
            &chatgpt_sized_credential(),
        )
        .unwrap();
        delete(slots(&store), "provider:openai").unwrap();

        assert!(store.entries.borrow().is_empty());
        assert!(matches!(
            read(slots(&store), "provider:openai"),
            Err(ChunkedError::Keyring(keyring::Error::NoEntry))
        ));
    }

    #[test]
    fn failed_delete_keeps_the_chunked_secret_readable() {
        let store = Rc::new(Store::default());
        let secret = chatgpt_sized_credential();
        write(slots(&store), "provider:openai", &secret).unwrap();
        *store.fail_account.borrow_mut() = Some("provider:openai".to_string());

        assert!(delete(slots(&store), "provider:openai").is_err());
        *store.fail_account.borrow_mut() = None;

        assert_eq!(read(slots(&store), "provider:openai").unwrap(), secret);
    }

    #[test]
    fn failed_chunked_write_keeps_the_previous_secret() {
        let store = Rc::new(Store::default());
        write(slots(&store), "provider:openai", "sk-old").unwrap();
        *store.fail_account.borrow_mut() = Some("provider:openai".to_string());

        assert!(
            write(
                slots(&store),
                "provider:openai",
                &chatgpt_sized_credential()
            )
            .is_err()
        );
        *store.fail_account.borrow_mut() = None;

        assert_eq!(store.entries.borrow().len(), 1);
        assert_eq!(read(slots(&store), "provider:openai").unwrap(), "sk-old");
    }

    #[test]
    fn missing_chunk_is_reported_as_incomplete() {
        let store = Rc::new(Store::default());
        write(
            slots(&store),
            "provider:openai",
            &chatgpt_sized_credential(),
        )
        .unwrap();
        let chunk = store
            .entries
            .borrow()
            .keys()
            .find(|account| account.contains(":chunk:"))
            .cloned()
            .unwrap();
        store.entries.borrow_mut().remove(&chunk);

        assert!(matches!(
            read(slots(&store), "provider:openai"),
            Err(ChunkedError::Incomplete)
        ));
    }

    #[test]
    fn splits_on_character_boundaries() {
        let value = "a😀".repeat(10);
        let chunks = split_utf16_chunks(&value, 4);

        assert_eq!(chunks.concat(), value);
        assert!(chunks.iter().all(|chunk| chunk.encode_utf16().count() <= 4));
    }

    #[test]
    fn ignores_values_that_only_resemble_a_manifest() {
        assert_eq!(
            Manifest::parse("anarlog-secure-store-chunks:v1:abc:0"),
            None
        );
        assert_eq!(Manifest::parse("anarlog-secure-store-chunks:v1::2"), None);
        assert_eq!(
            Manifest::parse("anarlog-secure-store-chunks:v1:1a2b:3"),
            Some(Manifest {
                generation: "1a2b".to_string(),
                count: 3,
            })
        );
    }
}
