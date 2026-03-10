#[cfg(target_os = "macos")]
use chrono::{DateTime, Utc};
use hypr_calendar_interface::{
    CalendarEvent, CalendarListItem, CalendarProviderType, CreateEventInput, EventFilter,
};
use hypr_google_calendar::{CalendarListEntry as GoogleCalendar, Event as GoogleEvent};
use hypr_outlook_calendar::{Calendar as OutlookCalendar, Event as OutlookEvent};
use tauri_plugin_auth::AuthPluginExt;
use tauri_plugin_permissions::PermissionsPluginExt;

use crate::error::Error;
use crate::fetch;

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ProviderConnectionIds {
    pub provider: CalendarProviderType,
    pub connection_ids: Vec<String>,
}

pub struct CalendarExt<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

pub fn available_providers() -> Vec<CalendarProviderType> {
    #[cfg(target_os = "macos")]
    let providers = vec![
        CalendarProviderType::Apple,
        CalendarProviderType::Google,
        CalendarProviderType::Outlook,
    ];

    #[cfg(not(target_os = "macos"))]
    let providers = vec![CalendarProviderType::Google, CalendarProviderType::Outlook];

    providers
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> CalendarExt<'a, R, M> {
    pub async fn list_calendars(
        &self,
        provider: CalendarProviderType,
        connection_id: String,
    ) -> Result<Vec<CalendarListItem>, Error> {
        match provider {
            CalendarProviderType::Apple => {
                let calendars = self.list_apple_calendars()?;
                Ok(crate::convert::convert_apple_calendars(calendars))
            }
            CalendarProviderType::Google => {
                let calendars = self.list_google_calendars(&connection_id).await?;
                Ok(crate::convert::convert_google_calendars(calendars))
            }
            CalendarProviderType::Outlook => {
                let calendars = self.list_outlook_calendars(&connection_id).await?;
                Ok(crate::convert::convert_outlook_calendars(calendars))
            }
        }
    }

    pub async fn list_events(
        &self,
        provider: CalendarProviderType,
        connection_id: String,
        filter: EventFilter,
    ) -> Result<Vec<CalendarEvent>, Error> {
        match provider {
            CalendarProviderType::Apple => {
                let events = self.list_apple_events(filter)?;
                Ok(crate::convert::convert_apple_events(events))
            }
            CalendarProviderType::Google => {
                let calendar_id = filter.calendar_tracking_id.clone();
                let events = self.list_google_events(&connection_id, filter).await?;
                Ok(crate::convert::convert_google_events(events, &calendar_id))
            }
            CalendarProviderType::Outlook => {
                let calendar_id = filter.calendar_tracking_id.clone();
                let events = self.list_outlook_events(&connection_id, filter).await?;
                Ok(crate::convert::convert_outlook_events(events, &calendar_id))
            }
        }
    }

    pub fn open_calendar(&self, provider: CalendarProviderType) -> Result<(), Error> {
        match provider {
            CalendarProviderType::Apple => self.open_apple_calendar(),
            _ => Err(Error::UnsupportedOperation {
                operation: "open_calendar",
                provider,
            }),
        }
    }

    pub fn create_event(
        &self,
        provider: CalendarProviderType,
        input: CreateEventInput,
    ) -> Result<String, Error> {
        match provider {
            CalendarProviderType::Apple => self.create_apple_event(input),
            _ => Err(Error::UnsupportedOperation {
                operation: "create_event",
                provider,
            }),
        }
    }

    pub async fn list_connection_ids(&self) -> Result<Vec<ProviderConnectionIds>, Error> {
        let mut result = Vec::new();

        #[cfg(target_os = "macos")]
        {
            let status = self
                .manager
                .permissions()
                .check(tauri_plugin_permissions::Permission::Calendar)
                .await
                .map_err(|e| Error::Api(e.to_string()))?;

            if matches!(
                status,
                tauri_plugin_permissions::PermissionStatus::Authorized
            ) {
                result.push(ProviderConnectionIds {
                    provider: CalendarProviderType::Apple,
                    connection_ids: vec!["apple".to_string()],
                });
            }
        }

        let token = match self.get_access_token() {
            Ok(token) => token,
            Err(_) => return Ok(result),
        };

        let config = self.manager.state::<crate::PluginConfig>();
        let all = fetch::list_all_connection_ids(&config.api_base_url, &token).await?;

        for (integration_id, connection_ids) in all {
            let provider = match integration_id.as_str() {
                "google-calendar" => CalendarProviderType::Google,
                "outlook-calendar" => CalendarProviderType::Outlook,
                _ => continue,
            };
            result.push(ProviderConnectionIds {
                provider,
                connection_ids,
            });
        }

        Ok(result)
    }

    pub async fn is_provider_enabled(&self, provider: CalendarProviderType) -> Result<bool, Error> {
        let all = self.list_connection_ids().await?;
        Ok(all
            .iter()
            .any(|p| p.provider == provider && !p.connection_ids.is_empty()))
    }

    fn get_access_token(&self) -> Result<String, Error> {
        let token = self
            .manager
            .access_token()
            .map_err(|e| Error::Auth(e.to_string()))?;

        match token {
            Some(token) if !token.is_empty() => Ok(token),
            _ => Err(Error::NotAuthenticated),
        }
    }

    async fn list_google_calendars(
        &self,
        connection_id: &str,
    ) -> Result<Vec<GoogleCalendar>, Error> {
        let token = self.get_access_token()?;
        let config = self.manager.state::<crate::PluginConfig>();
        fetch::list_google_calendars(&config.api_base_url, &token, connection_id).await
    }

    async fn list_google_events(
        &self,
        connection_id: &str,
        filter: EventFilter,
    ) -> Result<Vec<GoogleEvent>, Error> {
        let token = self.get_access_token()?;
        let config = self.manager.state::<crate::PluginConfig>();
        fetch::list_google_events(&config.api_base_url, &token, connection_id, filter).await
    }

    async fn list_outlook_calendars(
        &self,
        connection_id: &str,
    ) -> Result<Vec<OutlookCalendar>, Error> {
        let token = self.get_access_token()?;
        let config = self.manager.state::<crate::PluginConfig>();
        fetch::list_outlook_calendars(&config.api_base_url, &token, connection_id).await
    }

    async fn list_outlook_events(
        &self,
        connection_id: &str,
        filter: EventFilter,
    ) -> Result<Vec<OutlookEvent>, Error> {
        let token = self.get_access_token()?;
        let config = self.manager.state::<crate::PluginConfig>();
        fetch::list_outlook_events(&config.api_base_url, &token, connection_id, filter).await
    }

    #[cfg(target_os = "macos")]
    fn open_apple_calendar(&self) -> Result<(), Error> {
        let script = String::from(
            "
            tell application \"Calendar\"
                activate
                switch view to month view
                view calendar at current date
            end tell
        ",
        );

        std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn()
            .map_err(|e| Error::Apple(e.to_string()))?
            .wait()
            .map_err(|e| Error::Apple(e.to_string()))?;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn list_apple_calendars(
        &self,
    ) -> Result<Vec<hypr_apple_calendar::types::AppleCalendar>, Error> {
        let handle = hypr_apple_calendar::Handle::new();
        handle
            .list_calendars()
            .map_err(|e| Error::Apple(e.to_string()))
    }

    #[cfg(target_os = "macos")]
    fn list_apple_events(
        &self,
        filter: EventFilter,
    ) -> Result<Vec<hypr_apple_calendar::types::AppleEvent>, Error> {
        let handle = hypr_apple_calendar::Handle::new();
        let filter = hypr_apple_calendar::types::EventFilter {
            from: filter.from,
            to: filter.to,
            calendar_tracking_id: filter.calendar_tracking_id,
        };

        handle
            .list_events(filter)
            .map_err(|e| Error::Apple(e.to_string()))
    }

    #[cfg(target_os = "macos")]
    fn create_apple_event(&self, input: CreateEventInput) -> Result<String, Error> {
        let handle = hypr_apple_calendar::Handle::new();

        let start_date = parse_datetime(&input.started_at, "started_at")?;
        let end_date = parse_datetime(&input.ended_at, "ended_at")?;

        let input = hypr_apple_calendar::types::CreateEventInput {
            title: input.title,
            start_date,
            end_date,
            calendar_id: input.calendar_tracking_id,
            is_all_day: input.is_all_day,
            location: input.location,
            notes: input.notes,
            url: input.url,
        };

        handle
            .create_event(input)
            .map_err(|e| Error::Apple(e.to_string()))
    }

    #[cfg(not(target_os = "macos"))]
    fn open_apple_calendar(&self) -> Result<(), Error> {
        Err(Error::ProviderUnavailable {
            provider: CalendarProviderType::Apple,
        })
    }

    #[cfg(not(target_os = "macos"))]
    fn list_apple_calendars(
        &self,
    ) -> Result<Vec<hypr_apple_calendar::types::AppleCalendar>, Error> {
        Err(Error::ProviderUnavailable {
            provider: CalendarProviderType::Apple,
        })
    }

    #[cfg(not(target_os = "macos"))]
    fn list_apple_events(
        &self,
        _filter: EventFilter,
    ) -> Result<Vec<hypr_apple_calendar::types::AppleEvent>, Error> {
        Err(Error::ProviderUnavailable {
            provider: CalendarProviderType::Apple,
        })
    }

    #[cfg(not(target_os = "macos"))]
    fn create_apple_event(&self, _input: CreateEventInput) -> Result<String, Error> {
        Err(Error::ProviderUnavailable {
            provider: CalendarProviderType::Apple,
        })
    }
}

#[cfg(target_os = "macos")]
fn parse_datetime(value: &str, field: &'static str) -> Result<DateTime<Utc>, Error> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| Error::InvalidDateTime {
            field,
            value: value.to_string(),
        })
}

pub trait CalendarPluginExt<R: tauri::Runtime> {
    fn calendar(&self) -> CalendarExt<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> CalendarPluginExt<R> for T {
    fn calendar(&self) -> CalendarExt<'_, R, Self>
    where
        Self: Sized,
    {
        CalendarExt {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
