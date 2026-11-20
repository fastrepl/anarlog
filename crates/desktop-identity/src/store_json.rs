//! The desktop app's `<base>/store.json`, in the format `plugins/store2` reads and writes: a
//! JSON object of scopes, each a JSON-encoded string of an object of keys. The shipping app
//! keeps onboarding, pinned tabs, recently opened notes, the last seen version and tray
//! preferences there.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Map, Value};

use crate::Locations;
use crate::paths::STORE_FILENAME;

static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub struct StoreJson {
    path: PathBuf,
}

impl StoreJson {
    pub fn new(locations: &Locations) -> Self {
        Self::at(locations.base.join(STORE_FILENAME))
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `scope`'s `key`; `None` when the file, scope or key is missing, as in store2.
    pub fn get(&self, scope: &str, key: &str) -> std::io::Result<Option<Value>> {
        let Some(store) = self.read()? else {
            return Ok(None);
        };
        Ok(scope_object(&store, scope)?.and_then(|mut scope| scope.remove(key)))
    }

    pub fn set(&self, scope: &str, key: &str, value: Value) -> std::io::Result<()> {
        self.update(scope, key, |_| true, value).map(|_| ())
    }

    /// Writes `value` only while `key` still holds `expected`; `false` when it changed.
    pub fn compare_and_set(
        &self,
        scope: &str,
        key: &str,
        expected: Option<&Value>,
        value: Value,
    ) -> std::io::Result<bool> {
        self.update(scope, key, |current| current == expected, value)
    }

    fn update(
        &self,
        scope: &str,
        key: &str,
        accept: impl FnOnce(Option<&Value>) -> bool,
        value: Value,
    ) -> std::io::Result<bool> {
        let _guard = WRITE_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        // An unreadable store is left alone, as store2 refuses to load it rather than replace it.
        let mut store = self.read()?.unwrap_or_default();
        let mut object = scope_object(&store, scope)?.unwrap_or_default();
        if !accept(object.get(key)) {
            return Ok(false);
        }
        object.insert(key.to_string(), value);
        let encoded = serde_json::to_string(&object).map_err(invalid_data)?;
        store.insert(scope.to_string(), Value::String(encoded));
        let content = serde_json::to_string_pretty(&store).map_err(invalid_data)?;
        anlg_storage::fs::atomic_write(&self.path, &content)?;
        Ok(true)
    }

    fn read(&self) -> std::io::Result<Option<Map<String, Value>>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(invalid_data),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}

/// A scope that isn't a string reads as missing, and store2's next write replaces it.
fn scope_object(
    store: &Map<String, Value>,
    scope: &str,
) -> std::io::Result<Option<Map<String, Value>>> {
    let Some(Value::String(encoded)) = store.get(scope) else {
        return Ok(None);
    };
    match serde_json::from_str(encoded).map_err(invalid_data)? {
        Value::Object(object) => Ok(Some(object)),
        _ => Ok(None),
    }
}

fn invalid_data(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A `store.json` as store2 writes it.
    const SHIPPING: &str = r#"{
  "desktop": "{\"OnboardingNeeded2\":false,\"PinnedTabs\":\"[{\\\"type\\\":\\\"contacts\\\",\\\"pinned\\\":true}]\"}",
  "updater2": "{\"AutomaticUpdatesEnabled\":true,\"LastSeenVersion\":\"1.4.35\"}",
  "analytics": "{\"Disabled\":false}"
}"#;

    #[test]
    fn reads_shipping_scopes() {
        let root = tempfile::tempdir().unwrap();
        let store = StoreJson::at(root.path().join(STORE_FILENAME));
        assert_eq!(store.get("desktop", "OnboardingNeeded2").unwrap(), None);

        std::fs::write(store.path(), SHIPPING).unwrap();
        assert_eq!(
            store.get("desktop", "OnboardingNeeded2").unwrap(),
            Some(json!(false))
        );
        assert_eq!(
            store.get("desktop", "PinnedTabs").unwrap(),
            Some(json!(r#"[{"type":"contacts","pinned":true}]"#))
        );
        assert_eq!(
            store.get("updater2", "LastSeenVersion").unwrap(),
            Some(json!("1.4.35"))
        );
        assert_eq!(store.get("tray", "show_events_in_menu_bar").unwrap(), None);
    }

    #[test]
    fn writes_keep_other_scopes_and_keys_in_store2_format() {
        let root = tempfile::tempdir().unwrap();
        let store = StoreJson::at(root.path().join(STORE_FILENAME));
        std::fs::write(store.path(), SHIPPING).unwrap();

        store.set("desktop", "PinnedTabs", json!("[]")).unwrap();
        store
            .set("tray", "show_events_in_menu_bar", json!(false))
            .unwrap();

        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(store.path()).unwrap()).unwrap();
        let scope = |name: &str| -> Value {
            serde_json::from_str(written[name].as_str().expect("stringified scope")).unwrap()
        };
        assert_eq!(
            scope("desktop"),
            json!({"OnboardingNeeded2": false, "PinnedTabs": "[]"})
        );
        assert_eq!(scope("tray"), json!({"show_events_in_menu_bar": false}));
        assert_eq!(
            scope("updater2"),
            json!({"AutomaticUpdatesEnabled": true, "LastSeenVersion": "1.4.35"})
        );
        assert_eq!(written["analytics"], json!(r#"{"Disabled":false}"#));
    }

    #[test]
    fn compare_and_set_refuses_a_changed_value() {
        let root = tempfile::tempdir().unwrap();
        let store = StoreJson::at(root.path().join(STORE_FILENAME));
        assert!(
            store
                .compare_and_set("desktop", "PinnedTabs", None, json!("a"))
                .unwrap()
        );
        assert!(
            !store
                .compare_and_set("desktop", "PinnedTabs", None, json!("b"))
                .unwrap()
        );
        assert!(
            store
                .compare_and_set("desktop", "PinnedTabs", Some(&json!("a")), json!("b"))
                .unwrap()
        );
        assert_eq!(
            store.get("desktop", "PinnedTabs").unwrap(),
            Some(json!("b"))
        );
    }

    #[test]
    fn an_unreadable_store_is_never_replaced() {
        let root = tempfile::tempdir().unwrap();
        let store = StoreJson::at(root.path().join(STORE_FILENAME));
        std::fs::write(store.path(), "{ torn").unwrap();

        assert!(store.get("desktop", "OnboardingNeeded2").is_err());
        assert!(
            store
                .set("desktop", "OnboardingNeeded2", json!(false))
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(store.path()).unwrap(), "{ torn");
    }
}
