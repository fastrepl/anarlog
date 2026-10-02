use anlg_calendar_interface::CalendarProviderType;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::provider_str;
use super::transaction_utils::{js_iso8601_timestamp, rollback_error};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TombstoneCalendarConnectionRequest {
    pub provider: CalendarProviderType,
    pub connection_id: String,
}

pub async fn tombstone_calendar_connection(
    pool: &SqlitePool,
    request: TombstoneCalendarConnectionRequest,
) -> Result<(), String> {
    let provider = provider_str(request.provider);
    let now = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let result = async {
        anlg_db_app::tombstone_connection_events(
            &mut transaction,
            &now,
            provider,
            &request.connection_id,
        )
        .await
        .map_err(|error| error.to_string())?;
        anlg_db_app::tombstone_connection_calendars(
            &mut transaction,
            &now,
            provider,
            &request.connection_id,
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
        db
    }

    #[tokio::test]
    async fn tombstones_only_the_targeted_provider_connection() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO calendars (id, tracking_id_calendar, name, enabled, provider, connection_id)
             VALUES ('cal-personal', 'primary', 'Personal', 1, 'google', 'conn-personal'),
                    ('cal-work', 'primary', 'Work', 1, 'google', 'conn-work')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO events (id, tracking_id_event, calendar_id, title, started_at)
             VALUES ('e-personal', 't1', 'cal-personal', 'P', '2026-06-01T10:00:00.000Z'),
                    ('e-work', 't2', 'cal-work', 'W', '2026-06-01T10:00:00.000Z')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        tombstone_calendar_connection(
            db.pool(),
            TombstoneCalendarConnectionRequest {
                provider: CalendarProviderType::Google,
                connection_id: "conn-personal".to_string(),
            },
        )
        .await
        .unwrap();

        let states: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT c.id, c.deleted_at, e.deleted_at
             FROM calendars c LEFT JOIN events e ON e.calendar_id = c.id ORDER BY c.id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(states.len(), 2);
        assert!(states[0].1.is_some());
        assert!(states[0].2.is_some());
        assert!(states[1].1.is_none());
        assert!(states[1].2.is_none());
    }
}
