use sqlx::SqlitePool;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub(crate) struct CalendarItem {
    pub tracking_id: String,
    pub name: String,
    pub color: String,
    pub source: String,
}

#[derive(Debug)]
pub(crate) enum RuntimeEvent {
    CalendarPermissionStatus(CalendarPermissionState),
    CalendarPermissionResult(bool),
    CalendarPermissionReset,
    CalendarsLoaded(Vec<CalendarItem>),
    CalendarsSaved,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CalendarPermissionState {
    NotDetermined,
    Authorized,
    Denied,
}

#[cfg(target_os = "macos")]
fn map_auth_status(status: hypr_apple_calendar::CalendarAuthStatus) -> CalendarPermissionState {
    match status {
        hypr_apple_calendar::CalendarAuthStatus::NotDetermined => {
            CalendarPermissionState::NotDetermined
        }
        hypr_apple_calendar::CalendarAuthStatus::Authorized => CalendarPermissionState::Authorized,
        hypr_apple_calendar::CalendarAuthStatus::Denied => CalendarPermissionState::Denied,
    }
}

pub(crate) fn check_permission_sync() -> CalendarPermissionState {
    #[cfg(target_os = "macos")]
    {
        map_auth_status(hypr_apple_calendar::Handle::authorization_status())
    }
    #[cfg(not(target_os = "macos"))]
    {
        CalendarPermissionState::Denied
    }
}

#[cfg(target_os = "macos")]
fn to_calendar_item(cal: hypr_apple_calendar::types::AppleCalendar) -> CalendarItem {
    let color = cal
        .color
        .map(|c| {
            format!(
                "#{:02X}{:02X}{:02X}",
                (c.red * 255.0) as u8,
                (c.green * 255.0) as u8,
                (c.blue * 255.0) as u8
            )
        })
        .unwrap_or_default();
    CalendarItem {
        tracking_id: cal.id,
        name: cal.title,
        color,
        source: cal.source.title,
    }
}

pub(crate) fn load_calendars_sync() -> Result<Vec<CalendarItem>, String> {
    #[cfg(target_os = "macos")]
    {
        let handle = hypr_apple_calendar::Handle::new();
        handle
            .list_calendars()
            .map(|calendars| calendars.into_iter().map(to_calendar_item).collect())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Calendar listing is only available on macOS".to_string())
    }
}

pub(crate) struct Runtime {
    tx: mpsc::UnboundedSender<RuntimeEvent>,
}

impl Runtime {
    pub(crate) fn new(tx: mpsc::UnboundedSender<RuntimeEvent>) -> Self {
        Self { tx }
    }

    pub(crate) fn check_permission(&self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                let state = map_auth_status(hypr_apple_calendar::Handle::authorization_status());
                let _ = tx.send(RuntimeEvent::CalendarPermissionStatus(state));
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = tx.send(RuntimeEvent::Error(
                    "Calendar permissions are only available on macOS".to_string(),
                ));
            }
        });
    }

    pub(crate) fn request_permission(&self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                let granted = hypr_apple_calendar::Handle::request_full_access();
                let _ = tx.send(RuntimeEvent::CalendarPermissionResult(granted));
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = tx.send(RuntimeEvent::CalendarPermissionResult(false));
            }
        });
    }

    pub(crate) fn reset_permission(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = tokio::process::Command::new("tccutil")
                .args(["reset", "Calendar"])
                .output()
                .await;
            match result {
                Ok(_) => {
                    let _ = tx.send(RuntimeEvent::CalendarPermissionReset);
                }
                Err(e) => {
                    let _ = tx.send(RuntimeEvent::Error(e.to_string()));
                }
            }
        });
    }

    pub(crate) fn save_calendars(
        &self,
        pool: SqlitePool,
        provider: String,
        connection_id: String,
        items: Vec<(CalendarItem, bool)>,
    ) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            for (item, enabled) in &items {
                let id = format!("{connection_id}:{}", item.tracking_id);
                if let Err(e) = hypr_db_app::upsert_calendar(
                    &pool,
                    &id,
                    &provider,
                    &connection_id,
                    &item.tracking_id,
                    &item.name,
                    &item.color,
                    &item.source,
                    *enabled,
                )
                .await
                {
                    let _ = tx.send(RuntimeEvent::Error(e.to_string()));
                    return;
                }
            }
            let _ = tx.send(RuntimeEvent::CalendarsSaved);
        });
    }
}
