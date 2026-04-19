use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use hypr_calendar_interface::CalendarProviderType;
use hypr_calendar_sync::BoxError;

const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalendarSyncSnapshot {
    pub calendars: BTreeMap<String, CalendarRecord>,
    pub events: BTreeMap<String, EventRecord>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarRecord {
    pub user_id: String,
    pub created_at: String,
    pub tracking_id_calendar: String,
    pub name: String,
    pub enabled: bool,
    pub provider: CalendarProviderType,
    pub source: String,
    pub color: String,
    pub connection_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantRecord {
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_organizer: bool,
    pub is_current_user: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub user_id: String,
    pub created_at: String,
    pub tracking_id_event: Option<String>,
    pub calendar_id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub location: Option<String>,
    pub meeting_link: Option<String>,
    pub description: Option<String>,
    pub note: Option<String>,
    pub recurrence_series_id: Option<String>,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
    pub provider: CalendarProviderType,
    pub participants: Vec<ParticipantRecord>,
}

pub(crate) fn default_user_id() -> String {
    DEFAULT_USER_ID.to_string()
}
