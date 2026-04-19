use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use hypr_calendar::CalendarProviderType;
use hypr_calendar_sync::BoxError;

const CALENDARS_FILENAME: &str = "calendars.json";
const EVENTS_FILENAME: &str = "events.json";
const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalendarSyncSnapshot {
    pub calendars: BTreeMap<String, StoredCalendar>,
    pub events: BTreeMap<String, StoredEvent>,
}

pub trait CalendarSyncStore: Send + Sync + 'static {
    fn load_snapshot(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<CalendarSyncSnapshot, BoxError>> + Send + '_>>;

    fn save_snapshot(
        &self,
        snapshot: CalendarSyncSnapshot,
    ) -> Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send + '_>>;
}

#[derive(Clone)]
pub struct JsonCalendarSyncStore<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> JsonCalendarSyncStore<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: tauri::Runtime> CalendarSyncStore for JsonCalendarSyncStore<R> {
    fn load_snapshot(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<CalendarSyncSnapshot, BoxError>> + Send + '_>> {
        Box::pin(async move {
            let vault_base = resolve_vault_base(&self.app)?;
            let calendars =
                read_json_map::<StoredCalendar>(&vault_base.join(CALENDARS_FILENAME)).await?;
            let events = read_events_map(&vault_base.join(EVENTS_FILENAME)).await?;
            Ok(CalendarSyncSnapshot { calendars, events })
        })
    }

    fn save_snapshot(
        &self,
        snapshot: CalendarSyncSnapshot,
    ) -> Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send + '_>> {
        Box::pin(async move {
            let vault_base = resolve_vault_base(&self.app)?;
            write_json_map(&vault_base.join(CALENDARS_FILENAME), &snapshot.calendars).await?;
            write_events_map(&vault_base.join(EVENTS_FILENAME), &snapshot.events).await?;
            Ok(())
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoredCalendar {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoredParticipant {
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_organizer: bool,
    pub is_current_user: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    pub id: String,
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
    pub participants: Vec<StoredParticipant>,
}

pub fn default_user_id() -> String {
    DEFAULT_USER_ID.to_string()
}

impl serde::Serialize for StoredEvent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("user_id", &self.user_id)?;
        map.serialize_entry("created_at", &self.created_at)?;
        map.serialize_entry("tracking_id_event", &self.tracking_id_event)?;
        map.serialize_entry("calendar_id", &self.calendar_id)?;
        map.serialize_entry("title", &self.title)?;
        map.serialize_entry("started_at", &self.started_at)?;
        map.serialize_entry("ended_at", &self.ended_at)?;
        map.serialize_entry("location", &self.location)?;
        map.serialize_entry("meeting_link", &self.meeting_link)?;
        map.serialize_entry("description", &self.description)?;
        map.serialize_entry("note", &self.note)?;
        map.serialize_entry("recurrence_series_id", &self.recurrence_series_id)?;
        map.serialize_entry("has_recurrence_rules", &self.has_recurrence_rules)?;
        map.serialize_entry("is_all_day", &self.is_all_day)?;
        map.serialize_entry("provider", &self.provider)?;
        if !self.participants.is_empty() {
            map.serialize_entry("participants", &self.participants)?;
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for StoredEvent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct StoredEventData {
            #[serde(default = "default_user_id")]
            user_id: String,
            #[serde(default)]
            created_at: String,
            tracking_id_event: Option<String>,
            #[serde(default)]
            calendar_id: String,
            #[serde(default)]
            title: String,
            #[serde(default)]
            started_at: String,
            #[serde(default)]
            ended_at: Option<String>,
            #[serde(default)]
            location: Option<String>,
            #[serde(default)]
            meeting_link: Option<String>,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            note: Option<String>,
            #[serde(default)]
            recurrence_series_id: Option<String>,
            #[serde(default)]
            has_recurrence_rules: bool,
            #[serde(default)]
            is_all_day: bool,
            provider: CalendarProviderType,
            #[serde(default)]
            participants: Vec<StoredParticipant>,
        }

        let data = StoredEventData::deserialize(deserializer)?;
        Ok(Self {
            id: String::new(),
            user_id: data.user_id,
            created_at: data.created_at,
            tracking_id_event: data.tracking_id_event,
            calendar_id: data.calendar_id,
            title: data.title,
            started_at: data.started_at,
            ended_at: data.ended_at,
            location: data.location,
            meeting_link: data.meeting_link,
            description: data.description,
            note: data.note,
            recurrence_series_id: data.recurrence_series_id,
            has_recurrence_rules: data.has_recurrence_rules,
            is_all_day: data.is_all_day,
            provider: data.provider,
            participants: data.participants,
        })
    }
}

async fn read_json_map<T>(path: &Path) -> Result<BTreeMap<String, T>, BoxError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let content = match tokio::fs::read_to_string(path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(Box::new(error)),
    };

    Ok(serde_json::from_str(&content)?)
}

async fn read_events_map(path: &Path) -> Result<BTreeMap<String, StoredEvent>, BoxError> {
    let mut events = read_json_map::<StoredEvent>(path).await?;
    for (event_id, event) in &mut events {
        event.id = event_id.clone();
    }
    Ok(events)
}

async fn write_json_map<T>(path: &Path, value: &BTreeMap<String, T>) -> Result<(), BoxError>
where
    T: serde::Serialize,
{
    let content = serde_json::to_string(value)?;
    hypr_storage::fs::atomic_write_async(path, &content).await?;
    Ok(())
}

async fn write_events_map(
    path: &Path,
    value: &BTreeMap<String, StoredEvent>,
) -> Result<(), BoxError> {
    write_json_map(path, value).await
}

fn resolve_vault_base<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, BoxError> {
    let bundle_id: &str = app.config().identifier.as_ref();
    let settings_base = hypr_storage::global::compute_default_base(bundle_id)
        .ok_or_else(|| std::io::Error::other("settings base unavailable"))?;
    std::fs::create_dir_all(&settings_base)?;
    Ok(hypr_storage::vault::resolve_base(
        &settings_base,
        &settings_base,
    ))
}
