use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::transaction_utils::{js_iso8601_timestamp, rollback_error};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SetCalendarEnabledRequest {
    pub calendar_id: String,
    pub enabled: bool,
}

pub async fn set_calendar_enabled(
    pool: &SqlitePool,
    request: SetCalendarEnabledRequest,
) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let result = async {
        anlg_db_app::update_calendar_enabled(
            &mut transaction,
            request.enabled,
            &now,
            &request.calendar_id,
        )
        .await
        .map_err(|error| error.to_string())?;
        anlg_db_app::tombstone_events_when_calendar_disabled(
            &mut transaction,
            &now,
            &request.calendar_id,
            request.enabled,
        )
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }
    .await;

    if let Err(error) = result {
        return Err(rollback_error(transaction, error).await);
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO calendars (id, tracking_id_calendar, name, enabled, provider, connection_id)
             VALUES ('cal-1', 'primary', 'Work', 1, 'google', 'conn-1')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO events (id, tracking_id_event, calendar_id, title, started_at, ended_at)
             VALUES ('event-1', 'track-1', 'cal-1', 'Meeting', '2026-06-01T10:00:00.000Z', '')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn disabling_a_calendar_tombstones_its_events() {
        let db = test_db().await;

        set_calendar_enabled(
            db.pool(),
            SetCalendarEnabledRequest {
                calendar_id: "cal-1".to_string(),
                enabled: false,
            },
        )
        .await
        .unwrap();

        let enabled: i64 = sqlx::query_scalar("SELECT enabled FROM calendars WHERE id = 'cal-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let event_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM events WHERE id = 'event-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(enabled, 0);
        assert!(event_deleted.is_some());
    }

    #[tokio::test]
    async fn enabling_a_calendar_keeps_its_events() {
        let db = test_db().await;

        set_calendar_enabled(
            db.pool(),
            SetCalendarEnabledRequest {
                calendar_id: "cal-1".to_string(),
                enabled: true,
            },
        )
        .await
        .unwrap();

        let event_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM events WHERE id = 'event-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(event_deleted.is_none());
    }

    #[tokio::test]
    async fn already_deleted_rows_are_untouched() {
        let db = test_db().await;
        sqlx::query("UPDATE calendars SET deleted_at = 'gone' WHERE id = 'cal-1'")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE events SET deleted_at = 'gone' WHERE id = 'event-1'")
            .execute(db.pool())
            .await
            .unwrap();

        set_calendar_enabled(
            db.pool(),
            SetCalendarEnabledRequest {
                calendar_id: "cal-1".to_string(),
                enabled: false,
            },
        )
        .await
        .unwrap();

        let calendar_deleted: String =
            sqlx::query_scalar("SELECT deleted_at FROM calendars WHERE id = 'cal-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let event_deleted: String =
            sqlx::query_scalar("SELECT deleted_at FROM events WHERE id = 'event-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(calendar_deleted, "gone");
        assert_eq!(event_deleted, "gone");
    }
}
