use sqlx::SqlitePool;
use tokio::sync::mpsc;

use hypr_db_app::CalendarRow;

use super::app::Tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarPermissionState {
    NotDetermined,
    Authorized,
    Denied,
}

pub enum RuntimeEvent {
    SettingsLoaded {
        current_stt: Option<String>,
        current_llm: Option<String>,
        stt_providers: Vec<String>,
        llm_providers: Vec<String>,
    },
    CalendarsLoaded(Vec<CalendarRow>),
    CalendarPermissionStatus(CalendarPermissionState),
    Saved,
    Error(String),
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

pub struct Runtime {
    pool: SqlitePool,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
}

impl Runtime {
    pub fn new(pool: SqlitePool, tx: mpsc::UnboundedSender<RuntimeEvent>) -> Self {
        Self { pool, tx }
    }

    pub fn load_settings(&self) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let stt_connections = hypr_db_app::list_connections(&pool, Tab::Stt.connection_type())
                .await
                .unwrap_or_default();
            let llm_connections = hypr_db_app::list_connections(&pool, Tab::Llm.connection_type())
                .await
                .unwrap_or_default();

            let all_settings = hypr_db_app::load_all_settings(&pool)
                .await
                .unwrap_or_default();
            let map: std::collections::HashMap<String, String> = all_settings.into_iter().collect();

            let current_stt = map
                .get(Tab::Stt.setting_key())
                .filter(|v| !v.is_empty())
                .cloned();
            let current_llm = map
                .get(Tab::Llm.setting_key())
                .filter(|v| !v.is_empty())
                .cloned();

            let stt_providers: Vec<String> =
                stt_connections.into_iter().map(|c| c.provider_id).collect();
            let llm_providers: Vec<String> =
                llm_connections.into_iter().map(|c| c.provider_id).collect();

            let _ = tx.send(RuntimeEvent::SettingsLoaded {
                current_stt,
                current_llm,
                stt_providers,
                llm_providers,
            });
        });
    }

    pub fn save_provider(&self, tab: Tab, provider: String) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        let key = tab.setting_key();
        tokio::spawn(async move {
            match hypr_db_app::set_setting(&pool, key, &provider).await {
                Ok(()) => {
                    let _ = tx.send(RuntimeEvent::Saved);
                }
                Err(e) => {
                    let _ = tx.send(RuntimeEvent::Error(e.to_string()));
                }
            }
        });
    }

    pub fn load_calendars(&self) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let connections = hypr_db_app::list_connections(&pool, Tab::Calendar.connection_type())
                .await
                .unwrap_or_default();

            let mut db_calendars = Vec::new();
            for conn in &connections {
                if let Ok(cals) = hypr_db_app::list_calendars_by_connection(&pool, &conn.id).await {
                    db_calendars.extend(cals);
                }
            }

            #[cfg(target_os = "macos")]
            {
                if let Ok(fresh) = tokio::task::spawn_blocking(|| {
                    let handle = hypr_apple_calendar::Handle::new();
                    handle.list_calendars()
                })
                .await
                .unwrap_or_else(|_| Err(hypr_apple_calendar::Error::CalendarAccessDenied))
                {
                    let db_enabled: std::collections::HashMap<&str, bool> = db_calendars
                        .iter()
                        .map(|c| (c.tracking_id.as_str(), c.enabled))
                        .collect();

                    let connection_id = connections
                        .first()
                        .map(|c| c.id.as_str())
                        .unwrap_or("cal:apple_calendar");

                    let merged: Vec<CalendarRow> = fresh
                        .into_iter()
                        .map(|cal| {
                            let enabled = db_enabled.get(cal.id.as_str()).copied().unwrap_or(true);
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
                            CalendarRow {
                                id: format!("{connection_id}:{}", cal.id),
                                provider: "apple_calendar".to_string(),
                                connection_id: connection_id.to_string(),
                                tracking_id: cal.id,
                                name: cal.title,
                                color,
                                source: cal.source.title,
                                enabled,
                                created_at: String::new(),
                                user_id: String::new(),
                                raw_json: String::new(),
                            }
                        })
                        .collect();

                    let _ = tx.send(RuntimeEvent::CalendarsLoaded(merged));
                    return;
                }
            }

            let _ = tx.send(RuntimeEvent::CalendarsLoaded(db_calendars));
        });
    }

    pub fn save_calendars(&self, calendars: Vec<CalendarRow>) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            for cal in &calendars {
                if let Err(e) = hypr_db_app::upsert_calendar(
                    &pool,
                    &cal.id,
                    &cal.provider,
                    &cal.connection_id,
                    &cal.tracking_id,
                    &cal.name,
                    &cal.color,
                    &cal.source,
                    cal.enabled,
                )
                .await
                {
                    let _ = tx.send(RuntimeEvent::Error(e.to_string()));
                    return;
                }
            }
            let _ = tx.send(RuntimeEvent::Saved);
        });
    }

    pub fn check_permission(&self) {
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
}
