//! The shipping app keeps some state only in its web view's localStorage, which the native app
//! cannot read. The bridge release mirrors those entries, verbatim, to `<base>/webview-storage.json`
//! whenever they change and at startup; the native app imports them when it launches in their
//! place. Each write is a new snapshot `id`, so the importer can tell a newer mirror (written after
//! a rollback to the shipping app) from one it already merged.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value};

use crate::Locations;

pub const FILENAME: &str = "webview-storage.json";

/// `cloud-api/client.ts`: pending snapshot uploads and deletes, `{"upserts":[],"deletes":[]}`.
pub const CLOUD_API_PENDING: &str = "anarlog.cloud-api-pending.v1.";
/// `cloud-api/client.ts`: `"complete"` once the first upload of every meeting finished.
pub const CLOUD_API_BACKFILL: &str = "anarlog.cloud-api-backfill.v1.";
/// `cloud-api/lifecycle.tsx`: the last seen revision of each session, `[[id, revision], ...]`.
pub const CLOUD_API_REVISIONS: &str = "anarlog.cloud-api-revisions.v1.";
/// `auth/billing.tsx`: each trial dialog shown once per account, `"1"`.
pub const TRIAL_STARTED_SEEN: &str = "anarlog:trial_started_seen:";
pub const TRIAL_ENDED_SEEN: &str = "anarlog:trial_ended_seen:";
pub const TRIAL_PAYMENT_REMINDER_SEEN: &str = "anarlog:trial_payment_reminder_seen:";
/// `auth/cloudsync-progress.ts`: the initial sync's completion notice was shown, `"1"`.
pub const CLOUDSYNC_INITIAL_SYNC_COMPLETED: &str = "anarlog:cloudsync_initial_sync_completed:";
/// `shared/zoom.ts`: the UI zoom factor.
pub const ZOOM_FACTOR: &str = "anarlog-zoom-factor";

/// Every key mirrored, by prefix: only state the native app has a place for.
const MIRRORED: &[&str] = &[
    CLOUD_API_PENDING,
    CLOUD_API_BACKFILL,
    CLOUD_API_REVISIONS,
    TRIAL_STARTED_SEEN,
    TRIAL_ENDED_SEEN,
    TRIAL_PAYMENT_REMINDER_SEEN,
    CLOUDSYNC_INITIAL_SYNC_COMPLETED,
    ZOOM_FACTOR,
];

static WRITE_LOCK: Mutex<()> = Mutex::new(());
static WRITES: AtomicU64 = AtomicU64::new(0);

pub struct WebviewStorage {
    path: PathBuf,
}

/// One mirror of the web view's entries.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub id: String,
    pub entries: BTreeMap<String, String>,
}

impl Snapshot {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// The entries under `prefix`, keyed by the rest of the key (an account id, for most).
    pub fn with_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.entries
            .iter()
            .filter_map(move |(key, value)| Some((key.strip_prefix(prefix)?, value.as_str())))
    }
}

impl WebviewStorage {
    pub fn new(locations: &Locations) -> Self {
        Self::at(locations.base.join(FILENAME))
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Replaces the mirror with the mirrored keys among `entries` (the web view's whole
    /// localStorage) under a new snapshot id.
    pub fn write(
        &self,
        entries: impl IntoIterator<Item = (String, String)>,
    ) -> std::io::Result<()> {
        let entries: Map<String, Value> = entries
            .into_iter()
            .filter(|(key, _)| is_mirrored(key))
            .map(|(key, value)| (key, Value::String(value)))
            .collect();
        let _guard = WRITE_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let content = serde_json::json!({ "id": next_id(), "entries": entries });
        anlg_storage::fs::atomic_write(&self.path, &content.to_string())
    }

    /// The last mirror; `None` when the shipping app never wrote one.
    pub fn read(&self) -> std::io::Result<Option<Snapshot>> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(invalid_data)?;
        let id = value["id"]
            .as_str()
            .ok_or_else(|| invalid_data("the mirror has no id"))?;
        let entries = value["entries"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
            .collect();
        Ok(Some(Snapshot {
            id: id.to_owned(),
            entries,
        }))
    }
}

fn is_mirrored(key: &str) -> bool {
    MIRRORED.iter().any(|prefix| key.starts_with(prefix))
}

/// Unique across writes and launches; only ever compared for equality.
fn next_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{:x}-{write:x}", std::process::id())
}

fn invalid_data(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mirror_keeps_only_handoff_keys_and_each_write_is_a_new_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let storage = WebviewStorage::at(root.path().join(FILENAME));
        assert_eq!(storage.read().unwrap(), None);

        let pending = r#"{"upserts":[],"deletes":["note-1"]}"#;
        let entries = [
            (format!("{CLOUD_API_PENDING}user-1"), pending.to_owned()),
            (format!("{TRIAL_STARTED_SEEN}user-1"), "1".to_owned()),
            (ZOOM_FACTOR.to_owned(), "1.25".to_owned()),
            (
                "anarlog:pending-desktop-auth".to_owned(),
                "verifier".to_owned(),
            ),
            ("anarlog-theme".to_owned(), "dark".to_owned()),
        ];
        storage.write(entries.clone()).unwrap();
        let first = storage.read().unwrap().unwrap();
        assert_eq!(
            first.entries.keys().collect::<Vec<_>>(),
            [
                "anarlog-zoom-factor",
                "anarlog.cloud-api-pending.v1.user-1",
                "anarlog:trial_started_seen:user-1",
            ]
        );
        assert_eq!(
            first.with_prefix(CLOUD_API_PENDING).collect::<Vec<_>>(),
            [("user-1", pending)]
        );

        storage.write(entries).unwrap();
        let second = storage.read().unwrap().unwrap();
        assert_eq!(second.entries, first.entries);
        assert_ne!(second.id, first.id);
    }
}
