//! File-backed calendar sync store.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use chrono::Utc;
use hypr_calendar_sync::{BoxError, CalendarOp, CalendarPlan, EventOp, EventPlan};

use super::store::{
    CalendarRecord, EventRecord, ParticipantRecord, StoredCalendarRecord, StoredEventRecord,
    default_user_id,
};

const CALENDARS_FILENAME: &str = "calendars.json";
const EVENTS_FILENAME: &str = "events.json";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalendarSyncSnapshot {
    pub calendars: BTreeMap<String, CalendarRecord>,
    pub events: BTreeMap<String, EventRecord>,
}

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

    pub async fn load_snapshot(&self) -> Result<CalendarSyncSnapshot, BoxError> {
        self.read_snapshot().await
    }

    pub async fn mutate<F>(&self, mutator: F) -> Result<bool, BoxError>
    where
        F: FnOnce(&mut CalendarSyncSnapshot) -> bool + Send,
    {
        let _guard = self.write_lock.lock().await;
        let mut snapshot = self.read_snapshot().await?;
        let should_persist = mutator(&mut snapshot);
        if should_persist {
            self.write_snapshot(&snapshot).await?;
        }
        Ok(should_persist)
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

impl hypr_calendar_sync::CalendarSyncStore for JsonCalendarSyncStore {
    type Calendar = StoredCalendarRecord;
    type Event = StoredEventRecord;

    fn read(
        &self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<(Vec<Self::Calendar>, Vec<Self::Event>), BoxError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let snapshot = self.load_snapshot().await?;
            Ok((
                snapshot
                    .calendars
                    .into_iter()
                    .map(|(id, record)| StoredCalendarRecord { id, record })
                    .collect(),
                snapshot
                    .events
                    .into_iter()
                    .map(|(id, record)| StoredEventRecord { id, record })
                    .collect(),
            ))
        })
    }

    fn apply<'a>(
        &'a self,
        calendar_plan: CalendarPlan<'a>,
        event_plan: EventPlan<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<bool, BoxError>> + Send + 'a>> {
        Box::pin(async move {
            let _guard = self.write_lock.lock().await;
            let mut snapshot = self.read_snapshot().await?;
            let calendar_changed = apply_calendar_plan(&mut snapshot, calendar_plan);
            let event_changed = apply_event_plan(&mut snapshot, event_plan);
            let changed = calendar_changed || event_changed;
            if changed {
                self.write_snapshot(&snapshot).await?;
            }
            Ok(changed)
        })
    }
}

fn apply_calendar_plan(snapshot: &mut CalendarSyncSnapshot, plan: CalendarPlan<'_>) -> bool {
    let mut changed = false;

    for op in plan.ops {
        match op {
            CalendarOp::Delete { id } => {
                changed |= snapshot.calendars.remove(&id).is_some();
            }
            CalendarOp::Upsert {
                existing_id,
                incoming,
            } => {
                let row_id = existing_id.unwrap_or_else(new_id);
                let existing = snapshot.calendars.get(&row_id);
                let next = CalendarRecord {
                    user_id: existing
                        .map(|row| row.user_id.clone())
                        .unwrap_or_else(default_user_id),
                    created_at: existing
                        .map(|row| row.created_at.clone())
                        .unwrap_or_else(now_iso),
                    tracking_id_calendar: incoming.key.tracking_id.clone(),
                    name: incoming.payload.name.clone(),
                    enabled: existing.map(|row| row.enabled).unwrap_or(false),
                    provider: incoming.key.provider,
                    source: incoming.payload.source.clone(),
                    color: incoming.payload.color.clone(),
                    connection_id: incoming.key.connection_id.clone(),
                };
                if existing != Some(&next) {
                    snapshot.calendars.insert(row_id, next);
                    changed = true;
                }
            }
        }
    }

    changed
}

fn apply_event_plan(snapshot: &mut CalendarSyncSnapshot, plan: EventPlan<'_>) -> bool {
    let calendar_ids_by_key = snapshot
        .calendars
        .iter()
        .map(|(id, record)| {
            (
                hypr_calendar_sync::CalendarKey::new(
                    record.provider,
                    record.connection_id.clone(),
                    record.tracking_id_calendar.clone(),
                ),
                id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut changed = false;

    for op in plan.ops {
        match op {
            EventOp::Delete { id } => {
                changed |= snapshot.events.remove(&id).is_some();
            }
            EventOp::Update { id, incoming } => {
                let Some(existing) = snapshot.events.get(&id).cloned() else {
                    tracing::error!(
                        event_id = %id,
                        tracking_id_event = %incoming.tracking_id_event,
                        "calendar sync update skipped because the persisted event row is missing"
                    );
                    continue;
                };
                let next = event_record_from_incoming(
                    incoming,
                    existing.calendar_id.clone(),
                    Some(&existing),
                );
                if existing != next {
                    snapshot.events.insert(id, next);
                    changed = true;
                }
            }
            EventOp::Insert { incoming } => {
                let Some(calendar_id) = calendar_ids_by_key.get(&incoming.calendar_key).cloned()
                else {
                    tracing::error!(
                        provider = ?incoming.calendar_key.provider,
                        connection_id = %incoming.calendar_key.connection_id,
                        tracking_id_calendar = %incoming.calendar_key.tracking_id,
                        tracking_id_event = %incoming.tracking_id_event,
                        "calendar sync insert skipped because the target calendar row is missing"
                    );
                    continue;
                };
                snapshot.events.insert(
                    new_id(),
                    event_record_from_incoming(incoming, calendar_id, None),
                );
                changed = true;
            }
        }
    }

    changed
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
    // Calendar sync follows the same startup vault resolution as settings and
    // legacy import: the global config lives under `settings_base`, and the
    // default vault base is also `settings_base` until the user opts into a
    // custom vault location.
    Ok(hypr_storage::vault::resolve_base(
        &settings_base,
        &settings_base,
    ))
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn event_record_from_incoming(
    incoming: &hypr_calendar_sync::IncomingEvent,
    calendar_id: String,
    existing: Option<&EventRecord>,
) -> EventRecord {
    EventRecord {
        user_id: existing
            .map(|row| row.user_id.clone())
            .unwrap_or_else(default_user_id),
        created_at: existing
            .map(|row| row.created_at.clone())
            .unwrap_or_else(now_iso),
        tracking_id_event: Some(incoming.tracking_id_event.clone()),
        calendar_id,
        title: incoming.payload.title.clone().unwrap_or_default(),
        started_at: incoming.started_at.clone(),
        ended_at: incoming.ended_at.clone(),
        location: incoming.payload.location.clone(),
        meeting_link: incoming.payload.meeting_link.clone(),
        description: incoming.payload.description.clone(),
        note: existing.and_then(|row| row.note.clone()),
        recurrence_series_id: incoming.recurrence_series_id.clone(),
        has_recurrence_rules: incoming.has_recurrence_rules,
        is_all_day: incoming.is_all_day,
        provider: incoming.calendar_key.provider,
        participants: incoming
            .participants
            .iter()
            .map(|participant| ParticipantRecord {
                name: participant.name.clone(),
                email: participant.email.clone(),
                is_organizer: participant.is_organizer,
                is_current_user: participant.is_current_user,
            })
            .collect(),
    }
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
    use std::collections::{BTreeMap, BTreeSet};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::time::Duration;

    use hypr_calendar::CalendarProviderType;
    use hypr_calendar_sync::{
        BoxError, CalendarKey, CalendarPayload, CalendarSyncRuntime, CalendarSyncSource,
        CalendarSyncWorkerEvent, ConnectionKey, EventPayload, IncomingCalendar, IncomingEvent,
        IncomingParticipant, IncomingSnapshot, SyncRange,
    };
    use tokio::sync::Notify;

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

    fn sample_event(calendar_id: &str) -> EventRecord {
        EventRecord {
            user_id: default_user_id(),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            tracking_id_event: Some("evt-1".to_string()),
            calendar_id: calendar_id.to_string(),
            title: "Standup".to_string(),
            started_at: "2026-04-15T09:00:00Z".to_string(),
            ended_at: Some("2026-04-15T09:30:00Z".to_string()),
            location: Some("Room 1".to_string()),
            meeting_link: Some("https://meet.google.com/abc-defg-hij".to_string()),
            description: Some("Daily standup".to_string()),
            note: Some("keep me".to_string()),
            recurrence_series_id: None,
            has_recurrence_rules: false,
            is_all_day: false,
            provider: CalendarProviderType::Google,
            participants: vec![ParticipantRecord {
                name: Some("Alice".to_string()),
                email: Some("alice@example.com".to_string()),
                is_organizer: true,
                is_current_user: true,
            }],
        }
    }

    #[derive(Clone)]
    struct StaticSource {
        snapshot: IncomingSnapshot,
    }

    impl CalendarSyncSource for StaticSource {
        fn fetch(
            &self,
            _range: SyncRange,
        ) -> Pin<Box<dyn Future<Output = Result<IncomingSnapshot, BoxError>> + Send + '_>> {
            let snapshot = self.snapshot.clone();
            Box::pin(async move { Ok(snapshot) })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingRuntime {
        finished: Arc<Notify>,
        failure: Arc<std::sync::Mutex<Option<String>>>,
    }

    impl RecordingRuntime {
        fn failure(&self) -> Option<String> {
            self.failure.lock().unwrap().clone()
        }
    }

    impl CalendarSyncRuntime for RecordingRuntime {
        fn emit(&self, event: CalendarSyncWorkerEvent) {
            match event {
                CalendarSyncWorkerEvent::SyncFinished { .. } => self.finished.notify_waiters(),
                CalendarSyncWorkerEvent::SyncFailed { error } => {
                    *self.failure.lock().unwrap() = Some(error);
                    self.finished.notify_waiters();
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn mutate_skips_write_when_closure_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonCalendarSyncStore::from_base_path(dir.path().to_path_buf());

        let persisted = store
            .mutate(|snap| {
                snap.calendars
                    .insert("cal-1".to_string(), sample_calendar("cal-1"));
                false
            })
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
                        .mutate(move |snap| {
                            snap.calendars.insert(key.clone(), sample_calendar(&key));
                            true
                        })
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

    #[tokio::test]
    async fn apply_returns_false_for_noop_plans() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonCalendarSyncStore::from_base_path(dir.path().to_path_buf());

        store
            .mutate(|snapshot| {
                snapshot
                    .calendars
                    .insert("cal-1".to_string(), sample_calendar("primary"));
                snapshot
                    .events
                    .insert("event-1".to_string(), sample_event("cal-1"));
                true
            })
            .await
            .unwrap();

        let incoming_calendars = vec![IncomingCalendar {
            key: CalendarKey::new(CalendarProviderType::Apple, "conn", "primary"),
            payload: CalendarPayload {
                name: "primary".to_string(),
                source: "apple".to_string(),
                color: "#888".to_string(),
            },
        }];
        let incoming_events = vec![IncomingEvent {
            calendar_key: CalendarKey::new(CalendarProviderType::Google, "conn-1", "primary"),
            tracking_id_event: "evt-1".to_string(),
            started_at: "2026-04-15T09:00:00Z".to_string(),
            ended_at: Some("2026-04-15T09:30:00Z".to_string()),
            recurrence_series_id: None,
            has_recurrence_rules: false,
            is_all_day: false,
            participants: vec![IncomingParticipant {
                name: Some("Alice".to_string()),
                email: Some("alice@example.com".to_string()),
                is_organizer: true,
                is_current_user: true,
            }],
            payload: EventPayload {
                title: Some("Standup".to_string()),
                location: Some("Room 1".to_string()),
                meeting_link: Some("https://meet.google.com/abc-defg-hij".to_string()),
                description: Some("Daily standup".to_string()),
            },
        }];
        let existing_calendars = vec![StoredCalendarRecord {
            id: "cal-1".to_string(),
            record: sample_calendar("primary"),
        }];
        let requested_calendar_connections =
            BTreeSet::from([ConnectionKey::new(CalendarProviderType::Apple, "conn")]);
        let calendar_plan = hypr_calendar_sync::plan_calendars(
            &existing_calendars,
            &incoming_calendars,
            &requested_calendar_connections,
            &requested_calendar_connections,
        );
        let existing_event_calendars = vec![StoredCalendarRecord {
            id: "cal-1".to_string(),
            record: CalendarRecord {
                provider: CalendarProviderType::Google,
                source: "google".to_string(),
                color: "#4285f4".to_string(),
                connection_id: "conn-1".to_string(),
                ..sample_calendar("primary")
            },
        }];
        let existing_events = vec![StoredEventRecord {
            id: "event-1".to_string(),
            record: sample_event("cal-1"),
        }];
        let successful_event_connections =
            BTreeSet::from([ConnectionKey::new(CalendarProviderType::Google, "conn-1")]);
        let existing_event_calendar_plan = hypr_calendar_sync::CalendarPlan {
            ops: Vec::new(),
            enabled_calendar_ids: BTreeSet::from(["cal-1".to_string()]),
            enabled_calendar_keys: BTreeMap::from([(
                CalendarKey::new(CalendarProviderType::Google, "conn-1", "primary"),
                "cal-1".to_string(),
            )]),
            disabled_calendar_ids: BTreeSet::new(),
        };
        let event_plan = hypr_calendar_sync::plan_events(
            &existing_event_calendars,
            &existing_events,
            &incoming_events,
            &successful_event_connections,
            &existing_event_calendar_plan,
            SyncRange {
                from: chrono::DateTime::parse_from_rfc3339("2026-04-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc),
                to: chrono::DateTime::parse_from_rfc3339("2026-04-30T23:59:59Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            },
        );

        let changed = <JsonCalendarSyncStore as hypr_calendar_sync::CalendarSyncStore>::apply(
            &store,
            calendar_plan,
            event_plan,
        )
        .await
        .unwrap();

        assert!(!changed, "noop plans should not rewrite the snapshot");
    }

    #[tokio::test]
    async fn worker_persists_snapshot_end_to_end() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(JsonCalendarSyncStore::from_base_path(
            dir.path().to_path_buf(),
        ));
        store
            .mutate(|snapshot| {
                snapshot.calendars.insert(
                    "cal-primary".to_string(),
                    CalendarRecord {
                        user_id: default_user_id(),
                        created_at: "2026-04-15T00:00:00Z".to_string(),
                        tracking_id_calendar: "primary".to_string(),
                        name: "Primary".to_string(),
                        enabled: true,
                        provider: CalendarProviderType::Google,
                        source: "old@example.com".to_string(),
                        color: "#888".to_string(),
                        connection_id: "conn-1".to_string(),
                    },
                );
                true
            })
            .await
            .unwrap();

        let connection = ConnectionKey::new(CalendarProviderType::Google, "conn-1");
        let runtime = RecordingRuntime::default();
        let handle = hypr_calendar_sync::start(
            StaticSource {
                snapshot: IncomingSnapshot {
                    requested_connections: BTreeSet::from([connection.clone()]),
                    successful_calendar_connections: BTreeSet::from([connection.clone()]),
                    successful_event_connections: BTreeSet::from([connection.clone()]),
                    calendars: vec![IncomingCalendar {
                        key: CalendarKey::new(CalendarProviderType::Google, "conn-1", "primary"),
                        payload: CalendarPayload {
                            name: "Primary Renamed".to_string(),
                            source: "me@example.com".to_string(),
                            color: "#4285f4".to_string(),
                        },
                    }],
                    events: vec![IncomingEvent {
                        calendar_key: CalendarKey::new(
                            CalendarProviderType::Google,
                            "conn-1",
                            "primary",
                        ),
                        tracking_id_event: "evt-1".to_string(),
                        started_at: "2026-04-15T09:00:00Z".to_string(),
                        ended_at: Some("2026-04-15T09:30:00Z".to_string()),
                        recurrence_series_id: None,
                        has_recurrence_rules: false,
                        is_all_day: false,
                        participants: vec![IncomingParticipant {
                            name: Some("Alice".to_string()),
                            email: Some("alice@example.com".to_string()),
                            is_organizer: true,
                            is_current_user: true,
                        }],
                        payload: EventPayload {
                            title: Some("Standup".to_string()),
                            location: Some("Room 1".to_string()),
                            meeting_link: Some("https://meet.google.com/abc-defg-hij".to_string()),
                            description: Some("Daily standup".to_string()),
                        },
                    }],
                },
            },
            store.clone(),
            runtime.clone(),
            hypr_calendar_sync::Config {
                interval: Duration::from_secs(60 * 60),
                sync_timeout: Duration::from_secs(5),
            },
        );

        handle.request_sync().unwrap();
        tokio::time::timeout(Duration::from_secs(5), runtime.finished.notified())
            .await
            .expect("worker should finish a requested sync");

        assert_eq!(
            runtime.failure(),
            None,
            "worker should not emit sync failures"
        );

        let snapshot = store.load_snapshot().await.unwrap();
        let calendar = snapshot
            .calendars
            .get("cal-primary")
            .expect("existing calendar row should be updated");
        assert_eq!(calendar.name, "Primary Renamed");
        assert_eq!(calendar.source, "me@example.com");

        let event = snapshot
            .events
            .values()
            .next()
            .expect("worker should persist one event");
        assert_eq!(event.calendar_id, "cal-primary");
        assert_eq!(event.title, "Standup");
        assert_eq!(event.note.as_deref(), None);
        assert_eq!(event.participants.len(), 1);
    }
}
