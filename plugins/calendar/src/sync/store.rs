use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use hypr_calendar::CalendarProviderType;
use hypr_calendar_sync::BoxError;

const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalendarSyncSnapshot {
    pub calendars: BTreeMap<String, StoredCalendar>,
    pub events: BTreeMap<String, StoredEvent>,
}

pub type SnapshotMutator = Box<dyn FnOnce(&mut CalendarSyncSnapshot) -> bool + Send + 'static>;

/// Implementations must serialize `load_snapshot` and `mutate` against each
/// other on the same instance, and re-read the authoritative state inside
/// `mutate` so UI writes that land during a sync pass aren't clobbered.
pub trait CalendarSyncStore: Send + Sync + 'static {
    fn load_snapshot(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<CalendarSyncSnapshot, BoxError>> + Send + '_>>;

    /// Atomically load → apply `mutator` → save-if-changed under a per-store
    /// lock. The mutator returns `true` to persist, `false` to discard; the
    /// outer bool reports whether a write actually happened.
    fn mutate(
        &self,
        mutator: SnapshotMutator,
    ) -> Pin<Box<dyn Future<Output = Result<bool, BoxError>> + Send + '_>>;
}

// On-disk shape note: the JSON files were historically written by the old TS
// persister, which omitted any field it never set (e.g. `source` on a Google
// holidays calendar, `name` on a participant). Every field below — other than
// `provider`, where a missing value means data corruption — is `#[serde(default)]`
// so real user files still load. Do not relax this without a regression test.
// See `sync::json::tests::tolerates_legacy_ts_written_shape`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoredCalendar {
    #[serde(default = "default_user_id")]
    pub user_id: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub tracking_id_calendar: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    pub provider: CalendarProviderType,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub connection_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoredParticipant {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub is_organizer: bool,
    #[serde(default)]
    pub is_current_user: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoredEvent {
    #[serde(default = "default_user_id")]
    pub user_id: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub tracking_id_event: Option<String>,
    #[serde(default)]
    pub calendar_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub meeting_link: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub recurrence_series_id: Option<String>,
    #[serde(default)]
    pub has_recurrence_rules: bool,
    #[serde(default)]
    pub is_all_day: bool,
    pub provider: CalendarProviderType,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<StoredParticipant>,
}

pub(crate) fn default_user_id() -> String {
    DEFAULT_USER_ID.to_string()
}
