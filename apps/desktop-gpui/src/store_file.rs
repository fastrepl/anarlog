//! The desktop's `tauri-plugin-store2` file (`store.json` next to `app.db`):
//! `{"desktop": "<json>"}` where the inner document holds `StoreKey` values.
//! `RecentlyOpenedSessions` and `PinnedTabs` are JSON strings themselves.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

pub struct StoreFile {
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedSessionTab {
    pub id: String,
}

impl StoreFile {
    pub fn next_to(db_path: &Path) -> Self {
        Self {
            path: db_path.with_file_name("store.json"),
        }
    }

    fn read_desktop(&self) -> Map<String, Value> {
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return Map::new();
        };
        let Ok(outer) = serde_json::from_str::<Value>(&text) else {
            return Map::new();
        };
        outer
            .get("desktop")
            .and_then(Value::as_str)
            .and_then(|inner| serde_json::from_str::<Value>(inner).ok())
            .and_then(|inner| inner.as_object().cloned())
            .unwrap_or_default()
    }

    fn write_desktop(&self, desktop: &Map<String, Value>) -> std::io::Result<()> {
        let mut outer = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        let inner = serde_json::to_string(desktop).expect("json map serialises");
        outer.insert("desktop".into(), Value::String(inner));
        std::fs::write(
            &self.path,
            serde_json::to_string(&Value::Object(outer)).expect("json"),
        )
    }

    /// `loadRecentlyOpenedSessions`
    pub fn recently_opened_sessions(&self) -> Vec<String> {
        self.read_desktop()
            .get("RecentlyOpenedSessions")
            .and_then(Value::as_str)
            .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
            .unwrap_or_default()
    }

    /// `saveRecentlyOpenedSessions`
    pub fn save_recently_opened_sessions(&self, ids: &[String]) -> std::io::Result<()> {
        let mut desktop = self.read_desktop();
        desktop.insert(
            "RecentlyOpenedSessions".into(),
            Value::String(serde_json::to_string(ids).expect("json array")),
        );
        self.write_desktop(&desktop)
    }

    /// `loadPinnedTabs`, keeping the session tabs (the other pinned tab types
    /// have no surface in the shell yet).
    pub fn pinned_session_tabs(&self) -> Vec<PinnedSessionTab> {
        self.read_desktop()
            .get("PinnedTabs")
            .and_then(Value::as_str)
            .and_then(|json| serde_json::from_str::<Vec<Value>>(json).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|tab| tab.get("type").and_then(Value::as_str) == Some("sessions"))
            .filter_map(|tab| tab.get("id").and_then(Value::as_str).map(str::to_string))
            .map(|id| PinnedSessionTab { id })
            .collect()
    }

    /// `getDismissedToasts`
    pub fn dismissed_toasts(&self) -> Vec<String> {
        self.read_desktop()
            .get("DismissedToasts")
            .and_then(|value| serde_json::from_value::<Vec<String>>(value.clone()).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_the_double_encoded_desktop_document() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("app.db");
        let store = StoreFile::next_to(&db);
        assert!(store.recently_opened_sessions().is_empty());
        assert!(store.pinned_session_tabs().is_empty());

        std::fs::write(
            store.path.clone(),
            r#"{"desktop":"{\"OnboardingNeeded2\":false,\"RecentlyOpenedSessions\":\"[\\\"a\\\",\\\"b\\\"]\",\"PinnedTabs\":\"[{\\\"type\\\":\\\"sessions\\\",\\\"id\\\":\\\"a\\\",\\\"pinned\\\":true},{\\\"type\\\":\\\"calendar\\\",\\\"pinned\\\":true}]\",\"DismissedToasts\":[\"auth-promotion\"]}"}"#,
        )
        .unwrap();
        assert_eq!(store.recently_opened_sessions(), ["a", "b"]);
        assert_eq!(
            store.pinned_session_tabs(),
            [PinnedSessionTab { id: "a".into() }]
        );
        assert_eq!(store.dismissed_toasts(), ["auth-promotion"]);

        store
            .save_recently_opened_sessions(&["c".to_string(), "a".to_string()])
            .unwrap();
        assert_eq!(store.recently_opened_sessions(), ["c", "a"]);
        // Other keys survive a write, in the same double-encoded shape.
        let desktop = store.read_desktop();
        assert_eq!(desktop.get("OnboardingNeeded2"), Some(&Value::Bool(false)));
        assert!(desktop.get("PinnedTabs").is_some_and(Value::is_string));
        let outer: Value =
            serde_json::from_str(&std::fs::read_to_string(&store.path).unwrap()).unwrap();
        assert!(outer["desktop"].is_string());
    }
}
