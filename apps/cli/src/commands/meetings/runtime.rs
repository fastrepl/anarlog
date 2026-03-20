use chrono::Local;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

pub enum RuntimeEvent {
    MeetingsLoaded(Vec<hypr_db_app::MeetingRow>),
    EventsLoaded(Vec<hypr_db_app::EventRow>),
    CalendarNotConfigured,
    LoadError(String),
}

pub struct Runtime {
    pool: SqlitePool,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
}

impl Runtime {
    pub fn new(pool: SqlitePool, tx: mpsc::UnboundedSender<RuntimeEvent>) -> Self {
        Self { pool, tx }
    }

    pub fn load_meetings(&self) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match hypr_db_app::list_meetings(&pool).await {
                Ok(meetings) => {
                    let _ = tx.send(RuntimeEvent::MeetingsLoaded(meetings));
                }
                Err(e) => {
                    let _ = tx.send(RuntimeEvent::LoadError(e.to_string()));
                }
            }
        });
    }

    pub fn load_events(&self) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match hypr_db_app::has_calendars(&pool).await {
                Ok(true) => {
                    let today = Local::now().date_naive();
                    let start = today.format("%Y-%m-%d").to_string();
                    let end = (today + chrono::Duration::days(2))
                        .format("%Y-%m-%d")
                        .to_string();
                    match hypr_db_app::list_events_in_range(&pool, &start, &end).await {
                        Ok(events) => {
                            let _ = tx.send(RuntimeEvent::EventsLoaded(events));
                        }
                        Err(e) => {
                            let _ = tx.send(RuntimeEvent::LoadError(e.to_string()));
                        }
                    }
                }
                Ok(false) => {
                    let _ = tx.send(RuntimeEvent::CalendarNotConfigured);
                }
                Err(e) => {
                    let _ = tx.send(RuntimeEvent::LoadError(e.to_string()));
                }
            }
        });
    }
}
