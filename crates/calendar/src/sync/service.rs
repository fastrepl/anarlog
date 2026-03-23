use std::collections::HashMap;

use chrono::Utc;
use futures_util::stream::{self, StreamExt};
use uuid::Uuid;

use super::calendars::compute_calendar_sync;
use super::events::{ExistingEvent, IncomingParticipants, normalize_calendar_event};
use super::storage::{CalendarStorage, StoredCalendar, UpsertCalendar, UpsertEvent};
use crate::{CalendarProviderType, EventFilter, list_calendars, list_connection_ids, list_events};

const FETCH_CONCURRENCY: usize = 1;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("storage: {0}")]
    Storage(Box<dyn std::error::Error + Send + Sync>),
    #[error("provider: {0}")]
    Provider(#[from] crate::Error),
}

pub async fn run_sync(
    storage: &dyn CalendarStorage,
    api_base_url: &str,
    access_token: Option<&str>,
    apple_authorized: bool,
    user_id: &str,
) -> Result<(), SyncError> {
    let connections = list_connection_ids(api_base_url, access_token, apple_authorized).await?;

    for conn in &connections {
        let provider_str = provider_to_str(&conn.provider);
        if let Err(e) = sync_calendars_for_provider(
            storage,
            api_base_url,
            access_token.unwrap_or(""),
            &conn.provider,
            provider_str,
            &conn.connection_ids,
        )
        .await
        {
            tracing::error!(provider = provider_str, "calendar sync failed: {e}");
        }
    }

    for conn in &connections {
        let provider_str = provider_to_str(&conn.provider);
        for connection_id in &conn.connection_ids {
            if let Err(e) = sync_events_for_connection(
                storage,
                api_base_url,
                access_token.unwrap_or(""),
                &conn.provider,
                provider_str,
                connection_id,
                user_id,
            )
            .await
            {
                tracing::error!(
                    provider = provider_str,
                    connection_id,
                    "event sync failed: {e}"
                );
            }
        }
    }

    Ok(())
}

async fn sync_calendars_for_provider(
    storage: &dyn CalendarStorage,
    api_base_url: &str,
    access_token: &str,
    provider: &CalendarProviderType,
    provider_str: &str,
    connection_ids: &[String],
) -> Result<(), SyncError> {
    let mut per_connection = Vec::new();
    for connection_id in connection_ids {
        match list_calendars(api_base_url, access_token, provider.clone(), connection_id).await {
            Ok(cals) => per_connection.push((connection_id.clone(), cals)),
            Err(e) => {
                tracing::warn!(
                    provider = provider_str,
                    connection_id,
                    "failed to list calendars: {e}"
                );
                continue;
            }
        }
    }

    let existing = storage
        .list_all_calendars_for_provider(provider_str)
        .await
        .map_err(SyncError::Storage)?;

    let plan = compute_calendar_sync(&per_connection, &existing, provider_str);

    for id in &plan.to_delete {
        storage
            .delete_events_by_calendar(id)
            .await
            .map_err(SyncError::Storage)?;
        storage
            .delete_calendar(id)
            .await
            .map_err(SyncError::Storage)?;
    }

    let existing_by_tracking: HashMap<&str, &StoredCalendar> = existing
        .iter()
        .map(|c| (c.tracking_id.as_str(), c))
        .collect();

    for cal in &plan.to_upsert {
        let existing_cal = existing_by_tracking.get(cal.tracking_id.as_str());
        let id = existing_cal
            .map(|c| c.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let enabled = existing_cal.map(|c| c.enabled).unwrap_or(false);

        storage
            .upsert_calendar(UpsertCalendar {
                id,
                provider: cal.provider.clone(),
                connection_id: cal.connection_id.clone(),
                tracking_id: cal.tracking_id.clone(),
                name: cal.name.clone(),
                color: cal.color.clone(),
                source: cal.source.clone(),
                enabled,
            })
            .await
            .map_err(SyncError::Storage)?;
    }

    Ok(())
}

async fn sync_events_for_connection(
    storage: &dyn CalendarStorage,
    api_base_url: &str,
    access_token: &str,
    provider: &CalendarProviderType,
    provider_str: &str,
    connection_id: &str,
    user_id: &str,
) -> Result<(), SyncError> {
    let enabled_calendars = storage
        .list_enabled_calendars(provider_str, connection_id)
        .await
        .map_err(SyncError::Storage)?;

    if enabled_calendars.is_empty() {
        return Ok(());
    }

    let now = Utc::now();
    let from = now - chrono::Duration::days(7);
    let to = now + chrono::Duration::days(30);
    let from_str = from.to_rfc3339();
    let to_str = to.to_rfc3339();

    let calendar_ids: Vec<String> = enabled_calendars.iter().map(|c| c.id.clone()).collect();
    let tracking_to_local: HashMap<&str, &str> = enabled_calendars
        .iter()
        .map(|c| (c.tracking_id.as_str(), c.id.as_str()))
        .collect();

    let futures: Vec<_> = enabled_calendars
        .iter()
        .map(|cal| {
            let api_base_url = api_base_url.to_owned();
            let access_token = access_token.to_owned();
            let provider = provider.clone();
            let connection_id = connection_id.to_owned();
            let filter = EventFilter {
                from: from.into(),
                to: to.into(),
                calendar_tracking_id: cal.tracking_id.clone(),
            };
            let tracking_id = cal.tracking_id.clone();
            async move {
                let result = list_events(
                    &api_base_url,
                    &access_token,
                    provider,
                    &connection_id,
                    filter,
                )
                .await;
                (tracking_id, result)
            }
        })
        .collect();
    let fetch_results: Vec<_> = stream::iter(futures)
        .buffer_unordered(FETCH_CONCURRENCY)
        .collect()
        .await;

    let mut all_incoming = Vec::new();
    let mut all_participants: IncomingParticipants = HashMap::new();

    for (tracking_id, result) in fetch_results {
        let events = match result {
            Ok(events) => events,
            Err(e) => {
                tracing::warn!(
                    provider = provider_str,
                    calendar = tracking_id,
                    "failed to fetch events: {e}"
                );
                continue;
            }
        };

        for event in &events {
            if let Some((incoming, participants)) = normalize_calendar_event(event) {
                if !participants.is_empty() {
                    all_participants.insert(incoming.tracking_id_event.clone(), participants);
                }
                all_incoming.push(incoming);
            }
        }
    }

    let stored_events = storage
        .list_events_for_calendars(&calendar_ids, &from_str, &to_str)
        .await
        .map_err(SyncError::Storage)?;

    let existing: Vec<ExistingEvent> = stored_events
        .into_iter()
        .map(|e| ExistingEvent {
            id: e.id,
            user_id: e.user_id,
            calendar_id: e.calendar_id,
            tracking_id_event: e.tracking_id,
            title: e.title,
            started_at: e.started_at,
            ended_at: e.ended_at,
            location: e.location,
            meeting_link: e.meeting_link,
            description: e.description,
            note: e.note,
            recurrence_series_id: e.recurrence_series_id,
            has_recurrence_rules: e.has_recurrence_rules,
            is_all_day: e.is_all_day,
            participants_json: e.participants_json,
            raw_json: e.raw_json,
            created_at: e.created_at,
        })
        .collect();

    let plan = super::events::compute_sync_plan(&all_incoming, &existing, &all_participants);

    for id in &plan.to_delete {
        storage.delete_event(id).await.map_err(SyncError::Storage)?;
    }

    for update in &plan.to_update {
        let participants_json = serialize_participants(&update.participants);
        let calendar_id = tracking_to_local
            .get(update.incoming.tracking_id_calendar.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| update.calendar_id.clone());

        storage
            .upsert_event(UpsertEvent {
                id: update.id.clone(),
                user_id: update.user_id.clone(),
                calendar_id,
                tracking_id: update.incoming.tracking_id_event.clone(),
                title: update.incoming.title.clone(),
                started_at: update.incoming.started_at.clone(),
                ended_at: update.incoming.ended_at.clone(),
                location: update.incoming.location.clone(),
                meeting_link: update.incoming.meeting_link.clone(),
                description: update.incoming.description.clone(),
                note: update.note.clone(),
                recurrence_series_id: update.incoming.recurrence_series_id.clone(),
                has_recurrence_rules: update.incoming.has_recurrence_rules,
                is_all_day: update.incoming.is_all_day,
                participants_json,
                raw_json: String::new(),
            })
            .await
            .map_err(SyncError::Storage)?;
    }

    for add in &plan.to_add {
        let calendar_id = match tracking_to_local.get(add.incoming.tracking_id_calendar.as_str()) {
            Some(id) => id.to_string(),
            None => {
                tracing::warn!(
                    tracking_id = add.incoming.tracking_id_calendar,
                    "skipping event with unresolvable calendar"
                );
                continue;
            }
        };
        let participants_json = serialize_participants(&add.participants);

        storage
            .upsert_event(UpsertEvent {
                id: Uuid::new_v4().to_string(),
                user_id: user_id.to_string(),
                calendar_id,
                tracking_id: add.incoming.tracking_id_event.clone(),
                title: add.incoming.title.clone(),
                started_at: add.incoming.started_at.clone(),
                ended_at: add.incoming.ended_at.clone(),
                location: add.incoming.location.clone(),
                meeting_link: add.incoming.meeting_link.clone(),
                description: add.incoming.description.clone(),
                note: String::new(),
                recurrence_series_id: add.incoming.recurrence_series_id.clone(),
                has_recurrence_rules: add.incoming.has_recurrence_rules,
                is_all_day: add.incoming.is_all_day,
                participants_json,
                raw_json: String::new(),
            })
            .await
            .map_err(SyncError::Storage)?;
    }

    tracing::debug!(
        provider = provider_str,
        connection_id,
        added = plan.to_add.len(),
        updated = plan.to_update.len(),
        deleted = plan.to_delete.len(),
        "event sync complete"
    );

    Ok(())
}

fn serialize_participants(participants: &[super::events::EventParticipant]) -> String {
    if participants.is_empty() {
        return String::new();
    }
    #[derive(serde::Serialize)]
    struct P<'a> {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: &'a Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: &'a Option<String>,
        is_organizer: bool,
        is_current_user: bool,
    }
    let items: Vec<P> = participants
        .iter()
        .map(|p| P {
            name: &p.name,
            email: &p.email,
            is_organizer: p.is_organizer,
            is_current_user: p.is_current_user,
        })
        .collect();
    serde_json::to_string(&items).unwrap_or_default()
}

fn provider_to_str(provider: &CalendarProviderType) -> &'static str {
    match provider {
        CalendarProviderType::Apple => "apple",
        CalendarProviderType::Google => "google",
        CalendarProviderType::Outlook => "outlook",
    }
}
