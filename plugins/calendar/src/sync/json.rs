//! File-backed [`CalendarSyncStore`] impl: a JSON blob per table under a
//! per-store [`tokio::sync::Mutex`].

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use hypr_calendar_sync::BoxError;

use super::store::{
    CalendarRecord, CalendarSyncSnapshot, CalendarSyncStore, EventRecord, ParticipantRecord,
    SnapshotMutator, default_user_id,
};

const CALENDARS_FILENAME: &str = "calendars.json";
const EVENTS_FILENAME: &str = "events.json";

pub struct JsonCalendarSyncStore {
    base_path: PathBuf,
    write_lock: tokio::sync::Mutex<()>,
}

impl JsonCalendarSyncStore {
    pub fn from_base_path(base_path: PathBuf) -> Self {
        Self {
            base_path,
            write_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn for_app<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Self, BoxError> {
        Ok(Self::from_base_path(resolve_vault_base(app)?))
    }

    async fn read_snapshot(&self) -> Result<CalendarSyncSnapshot, BoxError> {
        let calendars =
            read_json_map::<JsonCalendarRecord>(&self.base_path.join(CALENDARS_FILENAME))
                .await?
                .into_iter()
                .map(|(id, record)| (id, record.into()))
                .collect();
        let events = read_json_map::<JsonEventRecord>(&self.base_path.join(EVENTS_FILENAME))
            .await?
            .into_iter()
            .map(|(id, record)| (id, record.into()))
            .collect();
        Ok(CalendarSyncSnapshot { calendars, events })
    }

    async fn write_snapshot(&self, snapshot: &CalendarSyncSnapshot) -> Result<(), BoxError> {
        let calendars = snapshot
            .calendars
            .iter()
            .map(|(id, record)| (id.clone(), JsonCalendarRecord::from(record)))
            .collect();
        let events = snapshot
            .events
            .iter()
            .map(|(id, record)| (id.clone(), JsonEventRecord::from(record)))
            .collect();
        write_json_map(&self.base_path.join(CALENDARS_FILENAME), &calendars).await?;
        write_json_map(&self.base_path.join(EVENTS_FILENAME), &events).await?;
        Ok(())
    }
}

impl CalendarSyncStore for JsonCalendarSyncStore {
    fn load_snapshot(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<CalendarSyncSnapshot, BoxError>> + Send + '_>> {
        Box::pin(async move {
            let _guard = self.write_lock.lock().await;
            self.read_snapshot().await
        })
    }

    fn mutate(
        &self,
        mutator: SnapshotMutator,
    ) -> Pin<Box<dyn Future<Output = Result<bool, BoxError>> + Send + '_>> {
        Box::pin(async move {
            let _guard = self.write_lock.lock().await;
            let mut snapshot = self.read_snapshot().await?;
            let should_persist = mutator(&mut snapshot);
            if should_persist {
                self.write_snapshot(&snapshot).await?;
            }
            Ok(should_persist)
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

async fn write_json_map<T>(path: &Path, value: &BTreeMap<String, T>) -> Result<(), BoxError>
where
    T: serde::Serialize,
{
    let content = serde_json::to_string(value)?;
    hypr_storage::fs::atomic_write_async(path, &content).await?;
    Ok(())
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

// On-disk shape note: the JSON files were historically written by the old TS
// persister, which omitted any field it never set (e.g. `source` on a Google
// holidays calendar, `name` on a participant). Every field below — other than
// `provider`, where a missing value means data corruption — is `#[serde(default)]`
// so real user files still load. Do not relax this without a regression test.
// See `tests::tolerates_legacy_ts_written_shape`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct JsonCalendarRecord {
    #[serde(default = "default_user_id")]
    user_id: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    tracking_id_calendar: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    enabled: bool,
    provider: hypr_calendar::CalendarProviderType,
    #[serde(default)]
    source: String,
    #[serde(default)]
    color: String,
    #[serde(default)]
    connection_id: String,
}

impl From<JsonCalendarRecord> for CalendarRecord {
    fn from(value: JsonCalendarRecord) -> Self {
        Self {
            user_id: value.user_id,
            created_at: value.created_at,
            tracking_id_calendar: value.tracking_id_calendar,
            name: value.name,
            enabled: value.enabled,
            provider: value.provider,
            source: value.source,
            color: value.color,
            connection_id: value.connection_id,
        }
    }
}

impl From<&CalendarRecord> for JsonCalendarRecord {
    fn from(value: &CalendarRecord) -> Self {
        Self {
            user_id: value.user_id.clone(),
            created_at: value.created_at.clone(),
            tracking_id_calendar: value.tracking_id_calendar.clone(),
            name: value.name.clone(),
            enabled: value.enabled,
            provider: value.provider,
            source: value.source.clone(),
            color: value.color.clone(),
            connection_id: value.connection_id.clone(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct JsonParticipantRecord {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    is_organizer: bool,
    #[serde(default)]
    is_current_user: bool,
}

impl From<JsonParticipantRecord> for ParticipantRecord {
    fn from(value: JsonParticipantRecord) -> Self {
        Self {
            name: value.name,
            email: value.email,
            is_organizer: value.is_organizer,
            is_current_user: value.is_current_user,
        }
    }
}

impl From<&ParticipantRecord> for JsonParticipantRecord {
    fn from(value: &ParticipantRecord) -> Self {
        Self {
            name: value.name.clone(),
            email: value.email.clone(),
            is_organizer: value.is_organizer,
            is_current_user: value.is_current_user,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct JsonEventRecord {
    #[serde(default = "default_user_id")]
    user_id: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
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
    provider: hypr_calendar::CalendarProviderType,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    participants: Vec<JsonParticipantRecord>,
}

impl From<JsonEventRecord> for EventRecord {
    fn from(value: JsonEventRecord) -> Self {
        Self {
            user_id: value.user_id,
            created_at: value.created_at,
            tracking_id_event: value.tracking_id_event,
            calendar_id: value.calendar_id,
            title: value.title,
            started_at: value.started_at,
            ended_at: value.ended_at,
            location: value.location,
            meeting_link: value.meeting_link,
            description: value.description,
            note: value.note,
            recurrence_series_id: value.recurrence_series_id,
            has_recurrence_rules: value.has_recurrence_rules,
            is_all_day: value.is_all_day,
            provider: value.provider,
            participants: value.participants.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<&EventRecord> for JsonEventRecord {
    fn from(value: &EventRecord) -> Self {
        Self {
            user_id: value.user_id.clone(),
            created_at: value.created_at.clone(),
            tracking_id_event: value.tracking_id_event.clone(),
            calendar_id: value.calendar_id.clone(),
            title: value.title.clone(),
            started_at: value.started_at.clone(),
            ended_at: value.ended_at.clone(),
            location: value.location.clone(),
            meeting_link: value.meeting_link.clone(),
            description: value.description.clone(),
            note: value.note.clone(),
            recurrence_series_id: value.recurrence_series_id.clone(),
            has_recurrence_rules: value.has_recurrence_rules,
            is_all_day: value.is_all_day,
            provider: value.provider,
            participants: value.participants.iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use hypr_calendar::CalendarProviderType;

    use super::*;

    fn sample_calendar(name: &str) -> CalendarRecord {
        CalendarRecord {
            user_id: default_user_id(),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            tracking_id_calendar: name.to_string(),
            name: name.to_string(),
            enabled: false,
            provider: CalendarProviderType::Apple,
            source: "apple".to_string(),
            color: "#888".to_string(),
            connection_id: "conn".to_string(),
        }
    }

    #[tokio::test]
    async fn mutate_skips_write_when_closure_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonCalendarSyncStore::from_base_path(dir.path().to_path_buf());

        let persisted = store
            .mutate(Box::new(|snap| {
                snap.calendars
                    .insert("cal-1".to_string(), sample_calendar("cal-1"));
                false
            }))
            .await
            .unwrap();

        assert!(!persisted);
        assert!(
            store.load_snapshot().await.unwrap().calendars.is_empty(),
            "discarded mutation must not touch disk",
        );
    }

    /// Regression: the previous schema required a literal `source` field on
    /// every calendar row, but the historical TS persister omitted it for
    /// rows where the provider didn't supply one (e.g. a Google "holidays"
    /// calendar). Loading such a file panicked with `missing field
    /// 'source'` and nuked the whole sync pass. Same deal for participants
    /// without a `name`.
    ///
    /// This fixture mirrors what was observed on a real user's disk —
    /// update it, don't relax it, if you change the shape.
    #[tokio::test]
    async fn tolerates_legacy_ts_written_shape() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join(CALENDARS_FILENAME),
            r##"{
              "holiday-row": {
                "color": "#16a765",
                "connection_id": "conn-1",
                "created_at": "2026-04-13T04:37:19.109Z",
                "enabled": false,
                "name": "Holidays",
                "provider": "google",
                "tracking_id_calendar": "holiday@group.v.calendar.google.com",
                "user_id": "00000000-0000-0000-0000-000000000000"
              },
              "primary-row": {
                "color": "#9fe1e7",
                "connection_id": "conn-1",
                "created_at": "2026-04-13T04:37:19.109Z",
                "enabled": true,
                "name": "me@example.com",
                "provider": "google",
                "source": "me@example.com",
                "tracking_id_calendar": "me@example.com",
                "user_id": "00000000-0000-0000-0000-000000000000"
              }
            }"##,
        )
        .await
        .unwrap();
        tokio::fs::write(
            dir.path().join(EVENTS_FILENAME),
            r##"{
              "ev-1": {
                "calendar_id": "primary-row",
                "created_at": "2026-04-13T04:37:41.864Z",
                "ended_at": "2026-04-07T20:15:00+09:00",
                "has_recurrence_rules": false,
                "is_all_day": false,
                "participants": [
                  {
                    "email": "me@example.com",
                    "is_current_user": true,
                    "is_organizer": true
                  }
                ],
                "provider": "google",
                "started_at": "2026-04-07T20:00:00+09:00",
                "title": "standup",
                "tracking_id_event": "evt-abc",
                "user_id": "00000000-0000-0000-0000-000000000000"
              }
            }"##,
        )
        .await
        .unwrap();

        let store = JsonCalendarSyncStore::from_base_path(dir.path().to_path_buf());
        let snapshot = store
            .load_snapshot()
            .await
            .expect("must tolerate legacy shape");

        let holiday = snapshot
            .calendars
            .get("holiday-row")
            .expect("holiday row must load");
        assert_eq!(holiday.source, "", "missing source must default, not error");
        assert_eq!(holiday.enabled, false);

        let event = snapshot.events.get("ev-1").expect("ev-1 must load");
        let participant = event.participants.first().expect("one participant");
        assert_eq!(
            participant.name, None,
            "missing participant name must default to None"
        );
        assert_eq!(participant.email.as_deref(), Some("me@example.com"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_mutations_never_lose_updates() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(JsonCalendarSyncStore::from_base_path(
            dir.path().to_path_buf(),
        ));

        const PER_TASK: usize = 25;

        let mut handles = Vec::with_capacity(2);
        for task_id in 0..2 {
            let store = store.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..PER_TASK {
                    let key = format!("task-{task_id}-cal-{i}");
                    store
                        .mutate(Box::new(move |snap| {
                            snap.calendars.insert(key.clone(), sample_calendar(&key));
                            true
                        }))
                        .await
                        .expect("mutate ok");
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let snapshot = store.load_snapshot().await.unwrap();
        assert_eq!(
            snapshot.calendars.len(),
            PER_TASK * 2,
            "all concurrent mutations must be preserved"
        );
    }
}
