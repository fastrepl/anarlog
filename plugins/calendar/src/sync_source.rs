use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use hypr_calendar::CalendarProviderType;
use hypr_calendar_interface::{AttendeeRole, CalendarEvent};
use hypr_calendar_sync::{BoxError, SyncOutcome};
use tauri::Manager;

use crate::auth::{access_token, is_apple_authorized, require_access_token};
use crate::sync_store::{
    CalendarSyncSnapshot, CalendarSyncStore, JsonCalendarSyncStore, StoredCalendar, StoredEvent,
    StoredParticipant, default_user_id,
};

#[derive(Clone)]
pub struct PluginCalendarSyncSource<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
    store: Arc<dyn CalendarSyncStore>,
}

impl<R: tauri::Runtime> PluginCalendarSyncSource<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self::with_store(app.clone(), JsonCalendarSyncStore::new(app))
    }

    pub fn with_store(app: tauri::AppHandle<R>, store: impl CalendarSyncStore) -> Self {
        Self {
            app,
            store: Arc::new(store),
        }
    }

    async fn sync_once(
        &self,
    ) -> Result<hypr_calendar_sync::SyncOutcome, hypr_calendar_sync::BoxError> {
        let api_base_url = self.app.state::<crate::PluginConfig>().api_base_url.clone();
        tracing::info!("calendar sync source: starting sync pass");
        let apple_authorized = is_apple_authorized(&self.app)
            .await
            .map_err(|error| Box::new(error) as BoxError)?;
        let provider_connections = hypr_calendar::list_connection_ids(
            &api_base_url,
            access_token(&self.app).as_deref(),
            apple_authorized,
        )
        .await?;
        tracing::info!(
            providers = provider_connections.len(),
            "calendar sync source: fetched provider connections"
        );

        let snapshot = self.store.load_snapshot().await?;
        let mut calendars = snapshot.calendars;
        let mut events = snapshot.events;
        tracing::info!(
            calendars = calendars.len(),
            events = events.len(),
            "calendar sync source: loaded snapshot"
        );

        let original_calendars = calendars.clone();
        let original_events = events.clone();

        for provider_connection_ids in &provider_connections {
            sync_calendars_for_provider(
                &self.app,
                &api_base_url,
                provider_connection_ids,
                &mut calendars,
                &mut events,
            )
            .await;
        }

        for provider_connection_ids in &provider_connections {
            for connection_id in &provider_connection_ids.connection_ids {
                if let Err(error) = sync_events_for_connection(
                    &self.app,
                    &api_base_url,
                    provider_connection_ids.provider,
                    connection_id,
                    &calendars,
                    &mut events,
                )
                .await
                {
                    tracing::error!(
                        provider = %provider_str(provider_connection_ids.provider),
                        connection_id,
                        "calendar sync failed for connection: {error}"
                    );
                }
            }
        }

        let calendars_changed = calendars != original_calendars;
        let events_changed = events != original_events;

        if calendars_changed || events_changed {
            self.store
                .save_snapshot(CalendarSyncSnapshot { calendars, events })
                .await?;
            tracing::info!(
                calendars_changed,
                events_changed,
                "calendar sync source: saved updated snapshot"
            );
        } else {
            tracing::info!("calendar sync source: snapshot unchanged");
        }

        Ok(SyncOutcome {
            data_changed: calendars_changed || events_changed,
        })
    }
}

impl<R: tauri::Runtime> hypr_calendar_sync::CalendarSyncSource for PluginCalendarSyncSource<R> {
    fn sync(
        &self,
        _reasons: Vec<hypr_calendar_sync::SyncReason>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<hypr_calendar_sync::SyncOutcome, hypr_calendar_sync::BoxError>,
                > + Send
                + '_,
        >,
    > {
        Box::pin(async move { self.sync_once().await })
    }
}

async fn sync_calendars_for_provider<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    api_base_url: &str,
    provider_connection_ids: &hypr_calendar::ProviderConnectionIds,
    calendars: &mut BTreeMap<String, StoredCalendar>,
    events: &mut BTreeMap<String, StoredEvent>,
) {
    let mut per_connection = Vec::new();
    for connection_id in &provider_connection_ids.connection_ids {
        match list_calendars(
            app,
            api_base_url,
            provider_connection_ids.provider,
            connection_id,
        )
        .await
        {
            Ok(connection_calendars) => {
                per_connection.push((connection_id.clone(), connection_calendars));
            }
            Err(error) => {
                tracing::warn!(
                    provider = %provider_str(provider_connection_ids.provider),
                    connection_id,
                    "failed to list calendars: {error}"
                );
            }
        }
    }

    let requested_connection_ids: BTreeSet<_> = provider_connection_ids
        .connection_ids
        .iter()
        .cloned()
        .collect();
    let successful_connection_ids: BTreeSet<_> = per_connection
        .iter()
        .map(|(connection_id, _)| connection_id.clone())
        .collect();
    let incoming_keys: BTreeSet<_> = per_connection
        .iter()
        .flat_map(|(connection_id, connection_calendars)| {
            connection_calendars.iter().map(|calendar| {
                calendar_tracking_key(
                    provider_connection_ids.provider,
                    connection_id,
                    &calendar.id,
                )
            })
        })
        .collect();

    let mut disabled_calendar_ids = BTreeSet::new();
    let existing_calendar_ids: Vec<_> = calendars.keys().cloned().collect();
    for calendar_id in existing_calendar_ids {
        let Some(calendar) = calendars.get(&calendar_id) else {
            continue;
        };
        if calendar.provider != provider_connection_ids.provider {
            continue;
        }

        let connection_id = calendar.connection_id.clone();
        let should_remove = !requested_connection_ids.contains(&connection_id)
            || (successful_connection_ids.contains(&connection_id)
                && !incoming_keys.contains(&calendar_tracking_key(
                    calendar.provider,
                    &connection_id,
                    &calendar.tracking_id_calendar,
                )));

        if should_remove {
            disabled_calendar_ids.insert(calendar_id.clone());
            calendars.remove(&calendar_id);
        } else if !calendar.enabled {
            disabled_calendar_ids.insert(calendar_id.clone());
        }
    }

    if !disabled_calendar_ids.is_empty() {
        events.retain(|_, event| !disabled_calendar_ids.contains(&event.calendar_id));
    }

    for (connection_id, connection_calendars) in per_connection {
        for calendar in connection_calendars {
            let existing_id = calendars.iter().find_map(|(calendar_id, row)| {
                (row.provider == provider_connection_ids.provider
                    && row.connection_id == connection_id
                    && row.tracking_id_calendar == calendar.id)
                    .then(|| calendar_id.clone())
            });

            let row_id = existing_id.clone().unwrap_or_else(new_id);
            let existing = existing_id
                .as_ref()
                .and_then(|calendar_id| calendars.get(calendar_id))
                .cloned();

            calendars.insert(
                row_id,
                StoredCalendar {
                    user_id: default_user_id(),
                    created_at: existing
                        .as_ref()
                        .map(|row| row.created_at.clone())
                        .unwrap_or_else(now_iso),
                    tracking_id_calendar: calendar.id,
                    name: calendar.title,
                    enabled: existing.as_ref().map(|row| row.enabled).unwrap_or(false),
                    provider: provider_connection_ids.provider,
                    source: calendar.source.unwrap_or_default(),
                    color: calendar.color.unwrap_or_else(|| "#888".to_string()),
                    connection_id: connection_id.clone(),
                },
            );
        }
    }
}

async fn sync_events_for_connection<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    api_base_url: &str,
    provider: CalendarProviderType,
    connection_id: &str,
    calendars: &BTreeMap<String, StoredCalendar>,
    events: &mut BTreeMap<String, StoredEvent>,
) -> Result<(), BoxError> {
    let enabled_calendar_ids: BTreeSet<_> = calendars
        .iter()
        .filter_map(|(calendar_id, calendar)| {
            (calendar.provider == provider
                && calendar.connection_id == connection_id
                && calendar.enabled)
                .then(|| calendar_id.clone())
        })
        .collect();

    let calendar_tracking_id_to_id: BTreeMap<_, _> = calendars
        .iter()
        .filter_map(|(calendar_id, calendar)| {
            (calendar.provider == provider
                && calendar.connection_id == connection_id
                && enabled_calendar_ids.contains(calendar_id))
            .then(|| (calendar.tracking_id_calendar.clone(), calendar_id.clone()))
        })
        .collect();

    let range = sync_range();
    let mut incoming_events = Vec::new();

    for tracking_id in calendar_tracking_id_to_id.keys() {
        let filter = hypr_calendar::EventFilter {
            from: range.from,
            to: range.to,
            calendar_tracking_id: tracking_id.clone(),
        };
        let fetched = list_events(app, api_base_url, provider, connection_id, filter).await?;
        for calendar_event in fetched {
            if should_skip_event(&calendar_event) {
                continue;
            }
            incoming_events.push(normalize_event(&calendar_event));
        }
    }

    let incoming_by_tracking_id: BTreeMap<_, _> = incoming_events
        .iter()
        .map(|event| (event.tracking_id_event.clone(), event.clone()))
        .collect();
    let existing_event_ids: Vec<_> = events.keys().cloned().collect();
    let mut handled_tracking_ids = BTreeSet::new();

    for event_id in existing_event_ids {
        let Some(existing) = events.get(&event_id).cloned() else {
            continue;
        };
        if !enabled_calendar_ids.contains(&existing.calendar_id) {
            continue;
        }
        if !is_event_in_range(
            &existing.started_at,
            existing.ended_at.as_deref(),
            range.from,
            range.to,
        ) {
            continue;
        }

        let Some(tracking_id) = existing.tracking_id_event.clone() else {
            events.remove(&event_id);
            continue;
        };

        if let Some(incoming) = incoming_by_tracking_id.get(&tracking_id) {
            let updated = StoredEvent {
                id: existing.id.clone(),
                user_id: existing.user_id.clone(),
                created_at: existing.created_at.clone(),
                tracking_id_event: Some(tracking_id.clone()),
                calendar_id: existing.calendar_id.clone(),
                title: incoming.title.clone().unwrap_or_default(),
                started_at: incoming.started_at.clone(),
                ended_at: incoming.ended_at.clone(),
                location: incoming.location.clone(),
                meeting_link: incoming.meeting_link.clone(),
                description: incoming.description.clone(),
                note: existing.note.clone(),
                recurrence_series_id: incoming.recurrence_series_id.clone(),
                has_recurrence_rules: incoming.has_recurrence_rules,
                is_all_day: incoming.is_all_day,
                provider,
                participants: incoming.participants.clone(),
            };
            events.insert(event_id, updated);
            handled_tracking_ids.insert(tracking_id);
        } else {
            events.remove(&event_id);
        }
    }

    for incoming in incoming_events {
        if handled_tracking_ids.contains(&incoming.tracking_id_event) {
            continue;
        }
        let Some(calendar_id) = calendar_tracking_id_to_id
            .get(&incoming.tracking_id_calendar)
            .cloned()
        else {
            continue;
        };

        let row_id = new_id();
        events.insert(
            row_id.clone(),
            StoredEvent {
                id: row_id,
                user_id: default_user_id(),
                created_at: now_iso(),
                tracking_id_event: Some(incoming.tracking_id_event),
                calendar_id,
                title: incoming.title.unwrap_or_default(),
                started_at: incoming.started_at,
                ended_at: incoming.ended_at,
                location: incoming.location,
                meeting_link: incoming.meeting_link,
                description: incoming.description,
                note: None,
                recurrence_series_id: incoming.recurrence_series_id,
                has_recurrence_rules: incoming.has_recurrence_rules,
                is_all_day: incoming.is_all_day,
                provider,
                participants: incoming.participants,
            },
        );
    }

    Ok(())
}

async fn list_calendars<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    api_base_url: &str,
    provider: CalendarProviderType,
    connection_id: &str,
) -> Result<Vec<hypr_calendar::CalendarListItem>, BoxError> {
    let token = match provider {
        CalendarProviderType::Apple => access_token(app).unwrap_or_default(),
        _ => require_access_token(app).map_err(|error| Box::new(error) as BoxError)?,
    };
    Ok(hypr_calendar::list_calendars(api_base_url, &token, provider, connection_id).await?)
}

async fn list_events<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    api_base_url: &str,
    provider: CalendarProviderType,
    connection_id: &str,
    filter: hypr_calendar::EventFilter,
) -> Result<Vec<CalendarEvent>, BoxError> {
    let token = match provider {
        CalendarProviderType::Apple => access_token(app).unwrap_or_default(),
        _ => require_access_token(app).map_err(|error| Box::new(error) as BoxError)?,
    };
    Ok(hypr_calendar::list_events(api_base_url, &token, provider, connection_id, filter).await?)
}

fn normalize_event(event: &CalendarEvent) -> IncomingEvent {
    let mut participants = Vec::new();
    if let Some(organizer) = &event.organizer {
        participants.push(StoredParticipant {
            name: organizer.name.clone(),
            email: organizer.email.clone(),
            is_organizer: true,
            is_current_user: organizer.is_current_user,
        });
    }

    let organizer_email = event
        .organizer
        .as_ref()
        .and_then(|organizer| organizer.email.as_ref())
        .map(|email| email.to_lowercase());
    for attendee in &event.attendees {
        if attendee.role == AttendeeRole::NonParticipant {
            continue;
        }
        if organizer_email.as_ref().is_some_and(|email| {
            attendee.email.as_ref().map(|value| value.to_lowercase()) == Some(email.clone())
        }) {
            continue;
        }
        participants.push(StoredParticipant {
            name: attendee.name.clone(),
            email: attendee.email.clone(),
            is_organizer: false,
            is_current_user: attendee.is_current_user,
        });
    }

    IncomingEvent {
        tracking_id_event: event.id.clone(),
        tracking_id_calendar: event.calendar_id.clone(),
        title: Some(event.title.clone()),
        started_at: event.started_at.clone(),
        ended_at: Some(event.ended_at.clone()),
        location: event.location.clone(),
        meeting_link: event
            .meeting_link
            .clone()
            .or_else(|| {
                event
                    .description
                    .as_deref()
                    .and_then(hypr_calendar::parse_meeting_link)
            })
            .or_else(|| {
                event
                    .location
                    .as_deref()
                    .and_then(hypr_calendar::parse_meeting_link)
            }),
        description: event.description.clone(),
        recurrence_series_id: event.recurring_event_id.clone(),
        has_recurrence_rules: event.has_recurrence_rules,
        is_all_day: event.is_all_day,
        participants,
    }
}

fn should_skip_event(event: &CalendarEvent) -> bool {
    event.attendees.iter().any(|attendee| {
        attendee.is_current_user
            && matches!(
                attendee.status,
                hypr_calendar_interface::AttendeeStatus::Declined
            )
    })
}

fn sync_range() -> SyncRange {
    let now = Utc::now();
    SyncRange {
        from: now - Duration::days(7),
        to: now + Duration::days(30),
    }
}

fn is_event_in_range(
    started_at: &str,
    ended_at: Option<&str>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> bool {
    let Ok(event_start) = DateTime::parse_from_rfc3339(started_at)
        .map(|value: DateTime<chrono::FixedOffset>| value.with_timezone(&Utc))
    else {
        return false;
    };
    let event_end = ended_at
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value: DateTime<chrono::FixedOffset>| value.with_timezone(&Utc))
        .unwrap_or(event_start);
    event_start <= to && event_end >= from
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn provider_str(provider: CalendarProviderType) -> &'static str {
    match provider {
        CalendarProviderType::Apple => "apple",
        CalendarProviderType::Google => "google",
        CalendarProviderType::Outlook => "outlook",
    }
}

fn calendar_tracking_key(
    provider: CalendarProviderType,
    connection_id: &str,
    tracking_id: &str,
) -> String {
    format!("{}:{connection_id}:{tracking_id}", provider_str(provider))
}

#[derive(Debug, Clone)]
struct IncomingEvent {
    tracking_id_event: String,
    tracking_id_calendar: String,
    title: Option<String>,
    started_at: String,
    ended_at: Option<String>,
    location: Option<String>,
    meeting_link: Option<String>,
    description: Option<String>,
    recurrence_series_id: Option<String>,
    has_recurrence_rules: bool,
    is_all_day: bool,
    participants: Vec<StoredParticipant>,
}

#[derive(Debug, Clone, Copy)]
struct SyncRange {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}
