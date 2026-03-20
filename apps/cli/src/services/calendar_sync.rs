use async_trait::async_trait;
use sqlx::SqlitePool;
use tokio::task::JoinHandle;

use hypr_calendar::sync::storage::{
    CalendarStorage, StoredCalendar, StoredEvent, UpsertCalendar, UpsertEvent,
};

pub struct SqliteCalendarStorage {
    pool: SqlitePool,
}

impl SqliteCalendarStorage {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CalendarStorage for SqliteCalendarStorage {
    async fn list_all_calendars_for_provider(
        &self,
        provider: &str,
    ) -> Result<Vec<StoredCalendar>, Box<dyn std::error::Error + Send + Sync>> {
        let rows = hypr_db_app::list_all_calendars_by_provider(&self.pool, provider).await?;
        Ok(rows
            .into_iter()
            .map(|r| StoredCalendar {
                id: r.id,
                provider: r.provider,
                connection_id: r.connection_id,
                tracking_id: r.tracking_id,
                name: r.name,
                color: r.color,
                source: r.source,
                enabled: r.enabled,
            })
            .collect())
    }

    async fn list_enabled_calendars(
        &self,
        provider: &str,
        connection_id: &str,
    ) -> Result<Vec<StoredCalendar>, Box<dyn std::error::Error + Send + Sync>> {
        let rows =
            hypr_db_app::list_enabled_calendars_by_provider(&self.pool, provider, connection_id)
                .await?;
        Ok(rows
            .into_iter()
            .map(|r| StoredCalendar {
                id: r.id,
                provider: r.provider,
                connection_id: r.connection_id,
                tracking_id: r.tracking_id,
                name: r.name,
                color: r.color,
                source: r.source,
                enabled: r.enabled,
            })
            .collect())
    }

    async fn list_events_for_calendars(
        &self,
        calendar_ids: &[String],
        from: &str,
        to: &str,
    ) -> Result<Vec<StoredEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let rows =
            hypr_db_app::list_events_by_calendar_ids(&self.pool, calendar_ids, from, to).await?;
        Ok(rows
            .into_iter()
            .map(|r| StoredEvent {
                id: r.id,
                user_id: r.user_id,
                calendar_id: r.calendar_id,
                tracking_id: r.tracking_id,
                title: r.title,
                started_at: r.started_at,
                ended_at: r.ended_at,
                location: r.location,
                meeting_link: r.meeting_link,
                description: r.description,
                note: r.note,
                recurrence_series_id: r.recurrence_series_id,
                has_recurrence_rules: r.has_recurrence_rules,
                is_all_day: r.is_all_day,
                participants_json: r.participants_json,
                raw_json: r.raw_json,
                created_at: r.created_at,
            })
            .collect())
    }

    async fn upsert_event(
        &self,
        event: UpsertEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        hypr_db_app::upsert_event(
            &self.pool,
            &event.id,
            &event.user_id,
            &event.calendar_id,
            &event.tracking_id,
            &event.title,
            &event.started_at,
            &event.ended_at,
            &event.location,
            &event.meeting_link,
            &event.description,
            &event.note,
            &event.recurrence_series_id,
            event.has_recurrence_rules,
            event.is_all_day,
            &event.participants_json,
            &event.raw_json,
        )
        .await?;
        Ok(())
    }

    async fn delete_event(&self, id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        hypr_db_app::delete_event(&self.pool, id).await?;
        Ok(())
    }

    async fn upsert_calendar(
        &self,
        cal: UpsertCalendar,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        hypr_db_app::upsert_calendar(
            &self.pool,
            &cal.id,
            &cal.provider,
            &cal.connection_id,
            &cal.tracking_id,
            &cal.name,
            &cal.color,
            &cal.source,
            cal.enabled,
        )
        .await?;
        Ok(())
    }

    async fn delete_calendar(
        &self,
        id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        hypr_db_app::delete_calendar(&self.pool, id).await?;
        Ok(())
    }

    async fn delete_events_by_calendar(
        &self,
        calendar_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        hypr_db_app::delete_events_by_calendar(&self.pool, calendar_id).await?;
        Ok(())
    }
}

pub struct CalendarSyncConfig {
    pub api_base_url: String,
    pub access_token: Option<String>,
    pub apple_authorized: bool,
    pub user_id: String,
}

#[derive(Clone)]
struct SyncState {
    storage: std::sync::Arc<SqliteCalendarStorage>,
    api_base_url: String,
    access_token: Option<String>,
    apple_authorized: bool,
    user_id: String,
}

async fn handle_tick(
    _tick: apalis_cron::Tick<chrono::Utc>,
    state: apalis::prelude::Data<SyncState>,
) {
    tracing::debug!("calendar sync tick");
    if let Err(e) = hypr_calendar::sync::service::run_sync(
        state.storage.as_ref(),
        &state.api_base_url,
        state.access_token.as_deref(),
        state.apple_authorized,
        &state.user_id,
    )
    .await
    {
        tracing::warn!("calendar sync error: {e}");
    }
}

pub fn spawn_calendar_sync(pool: SqlitePool, config: CalendarSyncConfig) -> JoinHandle<()> {
    tokio::spawn(async move {
        use std::str::FromStr;

        use apalis::prelude::*;
        use apalis_cron::CronStream;
        use cron::Schedule;

        let state = SyncState {
            storage: std::sync::Arc::new(SqliteCalendarStorage::new(pool)),
            api_base_url: config.api_base_url,
            access_token: config.access_token,
            apple_authorized: config.apple_authorized,
            user_id: config.user_id,
        };

        let schedule = Schedule::from_str("0 * * * * *").expect("valid cron expression");

        let worker = WorkerBuilder::new("calendar-sync")
            .backend(CronStream::new(schedule))
            .data(state)
            .build(handle_tick);

        if let Err(e) = worker.run().await {
            tracing::error!("calendar sync worker exited: {e}");
        }
    })
}
