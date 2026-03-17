mod convert;
mod error;
mod fetch;

pub use error::Error;
pub use hypr_calendar_interface::{
    CalendarEvent, CalendarListItem, CalendarProviderType, CreateEventInput, EventFilter,
};

#[cfg(target_os = "macos")]
pub use hypr_apple_calendar::setup_change_notification;

#[cfg(target_os = "macos")]
use chrono::{DateTime, Utc};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProviderConnectionIds {
    pub provider: CalendarProviderType,
    pub connection_ids: Vec<String>,
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

pub async fn list_connection_ids(
    api_base_url: &str,
    access_token: Option<&str>,
    apple_authorized: bool,
) -> Result<Vec<ProviderConnectionIds>, Error> {
    let mut result = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if apple_authorized {
            result.push(ProviderConnectionIds {
                provider: CalendarProviderType::Apple,
                connection_ids: vec!["apple".to_string()],
            });
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = apple_authorized;

    let token = match access_token {
        Some(t) if !t.is_empty() => t,
        _ => return Ok(result),
    };

    let all = fetch::list_all_connection_ids(api_base_url, token).await?;

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

pub async fn is_provider_enabled(
    api_base_url: &str,
    access_token: Option<&str>,
    apple_authorized: bool,
    provider: CalendarProviderType,
) -> Result<bool, Error> {
    let all = list_connection_ids(api_base_url, access_token, apple_authorized).await?;
    Ok(all
        .iter()
        .any(|p| p.provider == provider && !p.connection_ids.is_empty()))
}

pub async fn list_calendars(
    api_base_url: &str,
    access_token: &str,
    provider: CalendarProviderType,
    connection_id: &str,
) -> Result<Vec<CalendarListItem>, Error> {
    match provider {
        CalendarProviderType::Apple => {
            let calendars = list_apple_calendars()?;
            Ok(convert::convert_apple_calendars(calendars))
        }
        CalendarProviderType::Google => {
            let calendars =
                fetch::list_google_calendars(api_base_url, access_token, connection_id).await?;
            Ok(convert::convert_google_calendars(calendars))
        }
        CalendarProviderType::Outlook => {
            let calendars =
                fetch::list_outlook_calendars(api_base_url, access_token, connection_id).await?;
            Ok(convert::convert_outlook_calendars(calendars))
        }
    }
}

pub async fn list_events(
    api_base_url: &str,
    access_token: &str,
    provider: CalendarProviderType,
    connection_id: &str,
    filter: EventFilter,
) -> Result<Vec<CalendarEvent>, Error> {
    match provider {
        CalendarProviderType::Apple => {
            let events = list_apple_events(filter)?;
            Ok(convert::convert_apple_events(events))
        }
        CalendarProviderType::Google => {
            let calendar_id = filter.calendar_tracking_id.clone();
            let events =
                fetch::list_google_events(api_base_url, access_token, connection_id, filter)
                    .await?;
            Ok(convert::convert_google_events(events, &calendar_id))
        }
        CalendarProviderType::Outlook => {
            let calendar_id = filter.calendar_tracking_id.clone();
            let events =
                fetch::list_outlook_events(api_base_url, access_token, connection_id, filter)
                    .await?;
            Ok(convert::convert_outlook_events(events, &calendar_id))
        }
    }
}

pub fn open_calendar(provider: CalendarProviderType) -> Result<(), Error> {
    match provider {
        CalendarProviderType::Apple => open_apple_calendar(),
        _ => Err(Error::UnsupportedOperation {
            operation: "open_calendar",
            provider,
        }),
    }
}

pub fn create_event(
    provider: CalendarProviderType,
    input: CreateEventInput,
) -> Result<String, Error> {
    match provider {
        CalendarProviderType::Apple => create_apple_event(input),
        _ => Err(Error::UnsupportedOperation {
            operation: "create_event",
            provider,
        }),
    }
}

// --- Apple helpers ---

#[cfg(target_os = "macos")]
fn open_apple_calendar() -> Result<(), Error> {
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
fn list_apple_calendars() -> Result<Vec<hypr_apple_calendar::types::AppleCalendar>, Error> {
    let handle = hypr_apple_calendar::Handle::new();
    handle
        .list_calendars()
        .map_err(|e| Error::Apple(e.to_string()))
}

#[cfg(target_os = "macos")]
fn list_apple_events(
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
fn create_apple_event(input: CreateEventInput) -> Result<String, Error> {
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

#[cfg(target_os = "macos")]
fn parse_datetime(value: &str, field: &'static str) -> Result<DateTime<Utc>, Error> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| Error::InvalidDateTime {
            field,
            value: value.to_string(),
        })
}

#[cfg(not(target_os = "macos"))]
fn open_apple_calendar() -> Result<(), Error> {
    Err(Error::ProviderUnavailable {
        provider: CalendarProviderType::Apple,
    })
}

#[cfg(not(target_os = "macos"))]
fn list_apple_calendars() -> Result<Vec<hypr_apple_calendar::types::AppleCalendar>, Error> {
    Err(Error::ProviderUnavailable {
        provider: CalendarProviderType::Apple,
    })
}

#[cfg(not(target_os = "macos"))]
fn list_apple_events(
    _filter: EventFilter,
) -> Result<Vec<hypr_apple_calendar::types::AppleEvent>, Error> {
    Err(Error::ProviderUnavailable {
        provider: CalendarProviderType::Apple,
    })
}

#[cfg(not(target_os = "macos"))]
fn create_apple_event(_input: CreateEventInput) -> Result<String, Error> {
    Err(Error::ProviderUnavailable {
        provider: CalendarProviderType::Apple,
    })
}
