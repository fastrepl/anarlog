use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use hypr_calendar::CalendarProviderType;
use hypr_calendar_interface::{AttendeeRole, CalendarEvent};
use hypr_calendar_sync::{
    BoxError, CalendarKey, CalendarPayload, ConnectionKey, EventPayload, IncomingCalendar,
    IncomingEvent, IncomingParticipant, IncomingSnapshot, SyncRange,
};
use tauri::Manager;

use crate::auth::{access_token, is_apple_authorized, require_access_token};

use super::json::JsonCalendarSyncStore;

#[derive(Clone)]
pub struct PluginCalendarSyncSource<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
    store: Arc<JsonCalendarSyncStore>,
}

impl<R: tauri::Runtime> PluginCalendarSyncSource<R> {
    pub fn new(app: tauri::AppHandle<R>, store: Arc<JsonCalendarSyncStore>) -> Self {
        Self { app, store }
    }

    async fn fetch_snapshot(&self, range: SyncRange) -> Result<IncomingSnapshot, BoxError> {
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
        let stored_snapshot = self.store.load_snapshot().await?;
        let enabled_calendar_keys = enabled_calendar_keys(&stored_snapshot);

        let requested_connections = provider_connections
            .iter()
            .flat_map(|provider_connection_ids| {
                provider_connection_ids
                    .connection_ids
                    .iter()
                    .map(|connection_id| {
                        ConnectionKey::new(provider_connection_ids.provider, connection_id.clone())
                    })
            })
            .collect();

        let mut successful_calendar_connections = BTreeSet::new();
        let mut successful_event_connections = BTreeSet::new();
        let mut calendars = Vec::new();
        let mut events = Vec::new();

        for provider_connection_ids in &provider_connections {
            for connection_id in &provider_connection_ids.connection_ids {
                match list_calendars(
                    &self.app,
                    &api_base_url,
                    provider_connection_ids.provider,
                    connection_id,
                )
                .await
                {
                    Ok(connection_calendars) => {
                        successful_calendar_connections.insert(ConnectionKey::new(
                            provider_connection_ids.provider,
                            connection_id.clone(),
                        ));

                        for calendar in &connection_calendars {
                            calendars.push(IncomingCalendar {
                                key: CalendarKey::new(
                                    provider_connection_ids.provider,
                                    connection_id.clone(),
                                    calendar.id.clone(),
                                ),
                                payload: CalendarPayload {
                                    name: calendar.title.clone(),
                                    source: calendar.source.clone().unwrap_or_default(),
                                    color: calendar
                                        .color
                                        .clone()
                                        .unwrap_or_else(|| "#888".to_string()),
                                },
                            });
                        }

                        match fetch_events_for_connection(
                            &self.app,
                            &api_base_url,
                            provider_connection_ids.provider,
                            connection_id,
                            &connection_calendars,
                            &enabled_calendar_keys,
                            range,
                        )
                        .await
                        {
                            Ok(connection_events) => {
                                successful_event_connections.insert(ConnectionKey::new(
                                    provider_connection_ids.provider,
                                    connection_id.clone(),
                                ));
                                events.extend(connection_events);
                            }
                            Err(error) => {
                                tracing::error!(
                                    provider = %provider_str(provider_connection_ids.provider),
                                    connection_id,
                                    "calendar sync failed for connection: {error}"
                                );
                            }
                        }
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
        }

        Ok(IncomingSnapshot {
            requested_connections,
            successful_calendar_connections,
            successful_event_connections,
            calendars,
            events,
        })
    }
}

impl<R: tauri::Runtime> hypr_calendar_sync::CalendarSyncSource for PluginCalendarSyncSource<R> {
    fn fetch(
        &self,
        range: SyncRange,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        hypr_calendar_sync::IncomingSnapshot,
                        hypr_calendar_sync::BoxError,
                    >,
                > + Send
                + '_,
        >,
    > {
        Box::pin(async move { self.fetch_snapshot(range).await })
    }
}

async fn fetch_events_for_connection<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    api_base_url: &str,
    provider: CalendarProviderType,
    connection_id: &str,
    calendars: &[hypr_calendar::CalendarListItem],
    enabled_calendar_keys: &BTreeSet<CalendarKey>,
    range: SyncRange,
) -> Result<Vec<IncomingEvent>, BoxError> {
    let mut incoming_events = Vec::new();

    for calendar in
        filter_fetchable_calendars(provider, connection_id, calendars, enabled_calendar_keys)
    {
        let filter = hypr_calendar::EventFilter {
            from: range.from,
            to: range.to,
            calendar_tracking_id: calendar.id.clone(),
        };
        let fetched = list_events(app, api_base_url, provider, connection_id, filter).await?;
        for calendar_event in fetched {
            if should_skip_event(&calendar_event) {
                continue;
            }
            incoming_events.push(normalize_event(provider, connection_id, &calendar_event));
        }
    }

    Ok(incoming_events)
}

fn enabled_calendar_keys(snapshot: &super::json::CalendarSyncSnapshot) -> BTreeSet<CalendarKey> {
    snapshot
        .calendars
        .values()
        .filter(|calendar| calendar.enabled)
        .map(|calendar| {
            CalendarKey::new(
                calendar.provider,
                calendar.connection_id.clone(),
                calendar.tracking_id_calendar.clone(),
            )
        })
        .collect()
}

fn filter_fetchable_calendars<'a>(
    provider: CalendarProviderType,
    connection_id: &str,
    calendars: &'a [hypr_calendar::CalendarListItem],
    enabled_calendar_keys: &BTreeSet<CalendarKey>,
) -> Vec<&'a hypr_calendar::CalendarListItem> {
    calendars
        .iter()
        .filter(|calendar| {
            enabled_calendar_keys.contains(&CalendarKey::new(
                provider,
                connection_id,
                calendar.id.clone(),
            ))
        })
        .collect()
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

fn normalize_event(
    provider: CalendarProviderType,
    connection_id: &str,
    event: &CalendarEvent,
) -> IncomingEvent {
    let mut participants = Vec::new();
    if let Some(organizer) = &event.organizer {
        participants.push(IncomingParticipant {
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
        participants.push(IncomingParticipant {
            name: attendee.name.clone(),
            email: attendee.email.clone(),
            is_organizer: false,
            is_current_user: attendee.is_current_user,
        });
    }

    IncomingEvent {
        calendar_key: CalendarKey::new(
            provider,
            connection_id.to_string(),
            event.calendar_id.clone(),
        ),
        tracking_id_event: event.id.clone(),
        started_at: event.started_at.clone(),
        ended_at: Some(event.ended_at.clone()),
        recurrence_series_id: event.recurring_event_id.clone(),
        has_recurrence_rules: event.has_recurrence_rules,
        is_all_day: event.is_all_day,
        participants,
        payload: EventPayload {
            title: Some(event.title.clone()),
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
        },
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

fn provider_str(provider: CalendarProviderType) -> &'static str {
    match provider {
        CalendarProviderType::Apple => "apple",
        CalendarProviderType::Google => "google",
        CalendarProviderType::Outlook => "outlook",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::sync::json::CalendarSyncSnapshot;
    use crate::sync::store::CalendarRecord;

    #[test]
    fn disabled_calendars_stay_in_snapshot_but_are_not_fetchable() {
        let provider = CalendarProviderType::Google;
        let connection_id = "conn-1";
        let snapshot = CalendarSyncSnapshot {
            calendars: BTreeMap::from([
                (
                    "cal-enabled".to_string(),
                    CalendarRecord {
                        user_id: "user".to_string(),
                        created_at: "2026-04-15T00:00:00Z".to_string(),
                        tracking_id_calendar: "primary".to_string(),
                        name: "Primary".to_string(),
                        enabled: true,
                        provider,
                        source: "me@example.com".to_string(),
                        color: "#4285f4".to_string(),
                        connection_id: connection_id.to_string(),
                    },
                ),
                (
                    "cal-disabled".to_string(),
                    CalendarRecord {
                        user_id: "user".to_string(),
                        created_at: "2026-04-15T00:00:00Z".to_string(),
                        tracking_id_calendar: "holidays".to_string(),
                        name: "Holidays".to_string(),
                        enabled: false,
                        provider,
                        source: "holidays".to_string(),
                        color: "#16a765".to_string(),
                        connection_id: connection_id.to_string(),
                    },
                ),
            ]),
            events: BTreeMap::new(),
        };
        let provider_calendars = vec![
            test_calendar_list_item(provider, "primary", "Primary"),
            test_calendar_list_item(provider, "holidays", "Holidays"),
        ];

        let enabled_keys = enabled_calendar_keys(&snapshot);
        let fetchable =
            filter_fetchable_calendars(provider, connection_id, &provider_calendars, &enabled_keys);

        assert_eq!(
            snapshot.calendars.len(),
            2,
            "both calendars remain in snapshot"
        );
        assert_eq!(fetchable.len(), 1);
        assert_eq!(fetchable[0].id, "primary");
    }

    #[test]
    fn reenabled_calendar_becomes_fetchable() {
        let provider = CalendarProviderType::Google;
        let connection_id = "conn-1";
        let provider_calendars = vec![test_calendar_list_item(provider, "primary", "Primary")];
        let disabled_snapshot = CalendarSyncSnapshot {
            calendars: BTreeMap::from([(
                "cal-primary".to_string(),
                CalendarRecord {
                    user_id: "user".to_string(),
                    created_at: "2026-04-15T00:00:00Z".to_string(),
                    tracking_id_calendar: "primary".to_string(),
                    name: "Primary".to_string(),
                    enabled: false,
                    provider,
                    source: "me@example.com".to_string(),
                    color: "#4285f4".to_string(),
                    connection_id: connection_id.to_string(),
                },
            )]),
            events: BTreeMap::new(),
        };
        let enabled_snapshot = CalendarSyncSnapshot {
            calendars: BTreeMap::from([(
                "cal-primary".to_string(),
                CalendarRecord {
                    enabled: true,
                    ..disabled_snapshot.calendars["cal-primary"].clone()
                },
            )]),
            events: BTreeMap::new(),
        };

        let disabled_fetchable = filter_fetchable_calendars(
            provider,
            connection_id,
            &provider_calendars,
            &enabled_calendar_keys(&disabled_snapshot),
        );
        let enabled_fetchable = filter_fetchable_calendars(
            provider,
            connection_id,
            &provider_calendars,
            &enabled_calendar_keys(&enabled_snapshot),
        );

        assert!(disabled_fetchable.is_empty());
        assert_eq!(enabled_fetchable.len(), 1);
        assert_eq!(enabled_fetchable[0].id, "primary");
    }

    fn test_calendar_list_item(
        provider: CalendarProviderType,
        id: &str,
        title: &str,
    ) -> hypr_calendar::CalendarListItem {
        hypr_calendar::CalendarListItem {
            provider,
            id: id.to_string(),
            title: title.to_string(),
            source: None,
            color: None,
            is_primary: None,
            can_edit: None,
            raw: "{}".to_string(),
        }
    }
}
