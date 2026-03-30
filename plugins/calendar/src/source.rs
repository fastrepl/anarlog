use std::pin::Pin;

use chrono::{DateTime, Duration, Utc};
use hypr_calendar_interface::EventFilter;
use hypr_calendar_worker::source::{EventSource, UpcomingEvent};
use tauri::Manager;
use tauri_plugin_auth::AuthPluginExt;

pub struct CalendarEventSource<R: tauri::Runtime>(pub tauri::AppHandle<R>);

impl<R: tauri::Runtime> EventSource for CalendarEventSource<R> {
    fn upcoming_events(
        &self,
        within: Duration,
    ) -> Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<Vec<UpcomingEvent>, hypr_calendar_worker::Error>,
                > + Send
                + '_,
        >,
    > {
        let app = self.0.clone();
        Box::pin(async move {
            let config = app.state::<crate::PluginConfig>();
            let token = app.access_token().ok().flatten().filter(|t| !t.is_empty());

            let apple_authorized = {
                #[cfg(target_os = "macos")]
                {
                    use tauri_plugin_permissions::{
                        Permission, PermissionStatus, PermissionsPluginExt,
                    };
                    app.permissions()
                        .check(Permission::Calendar)
                        .await
                        .map(|s| matches!(s, PermissionStatus::Authorized))
                        .unwrap_or(false)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    false
                }
            };

            let connection_ids = match hypr_calendar::list_connection_ids(
                &config.api_base_url,
                token.as_deref(),
                apple_authorized,
            )
            .await
            {
                Ok(ids) => ids,
                Err(e) => {
                    tracing::warn!("calendar-worker: failed to list connection ids: {e}");
                    return Ok(Vec::new());
                }
            };

            let now = Utc::now();
            // Look back 2 minutes so a cold start catches meetings that just began.
            let from = now - chrono::Duration::minutes(2);
            let to = now + within;
            let mut upcoming = Vec::new();

            for provider_conn in connection_ids {
                let token_str = token.clone().unwrap_or_default();

                for connection_id in &provider_conn.connection_ids {
                    let calendars = match hypr_calendar::list_calendars(
                        &config.api_base_url,
                        &token_str,
                        provider_conn.provider,
                        connection_id,
                    )
                    .await
                    {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!("calendar-worker: failed to list calendars: {e}");
                            continue;
                        }
                    };

                    for calendar in calendars {
                        let filter = EventFilter {
                            from,
                            to,
                            calendar_tracking_id: calendar.id,
                        };

                        let events = match hypr_calendar::list_events(
                            &config.api_base_url,
                            &token_str,
                            provider_conn.provider,
                            connection_id,
                            filter,
                        )
                        .await
                        {
                            Ok(e) => e,
                            Err(e) => {
                                tracing::warn!("calendar-worker: failed to list events: {e}");
                                continue;
                            }
                        };

                        for event in events {
                            if event.is_all_day {
                                continue;
                            }

                            let Ok(started_at) = DateTime::parse_from_rfc3339(&event.started_at)
                                .map(|dt| dt.with_timezone(&Utc))
                            else {
                                continue;
                            };

                            let ended_at = DateTime::parse_from_rfc3339(&event.ended_at)
                                .map(|dt| dt.with_timezone(&Utc))
                                .ok();

                            let participants = event
                                .attendees
                                .iter()
                                .filter_map(|a| a.name.clone().or_else(|| a.email.clone()))
                                .collect();

                            upcoming.push(UpcomingEvent {
                                event_id: event.id,
                                title: event.title,
                                started_at,
                                ended_at,
                                participants,
                            });
                        }
                    }
                }
            }

            Ok(upcoming)
        })
    }
}
