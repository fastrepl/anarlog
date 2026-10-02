use std::collections::HashSet;

use anlg_calendar_interface::{CalendarListItem, CalendarProviderType};
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::transaction_utils::{js_iso8601_timestamp, rollback_error};
use indexmap::IndexMap;

use super::provider_str;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CalendarInventoryConnection {
    pub connection_id: String,
    pub calendars: Vec<CalendarListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApplyCalendarInventoryRequest {
    pub provider: CalendarProviderType,
    pub requested_connection_ids: Vec<String>,
    pub successful_connections: Vec<CalendarInventoryConnection>,
}

fn tracking_key(provider: &str, connection_id: &str, tracking_id: &str) -> String {
    [provider, connection_id, tracking_id].join(":")
}

pub async fn apply_calendar_inventory(
    pool: &SqlitePool,
    request: ApplyCalendarInventoryRequest,
) -> Result<(), String> {
    let provider = provider_str(request.provider);

    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let result = run(&mut transaction, &request, provider).await;
    if let Err(error) = result {
        return Err(rollback_error(transaction, error).await);
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

async fn run(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request: &ApplyCalendarInventoryRequest,
    provider: &str,
) -> Result<(), String> {
    let existing = anlg_db_app::list_provider_calendars(transaction, provider)
        .await
        .map_err(|error| error.to_string())?;

    let mut existing_by_tracking_key: IndexMap<String, &anlg_db_app::CalendarSyncRow> =
        IndexMap::new();
    for calendar in &existing {
        let key = tracking_key(
            &calendar.provider,
            &calendar.connection_id,
            &calendar.tracking_id_calendar,
        );
        let prefer = match existing_by_tracking_key.get(&key) {
            None => true,
            Some(current) => current.deleted_at.is_some() && calendar.deleted_at.is_none(),
        };
        if prefer {
            existing_by_tracking_key.insert(key, calendar);
        }
    }

    let requested: HashSet<&str> = request
        .requested_connection_ids
        .iter()
        .map(String::as_str)
        .collect();
    let successful: HashSet<&str> = request
        .successful_connections
        .iter()
        .map(|connection| connection.connection_id.as_str())
        .collect();
    let incoming_keys: HashSet<String> = request
        .successful_connections
        .iter()
        .flat_map(|connection| {
            connection.calendars.iter().map(move |calendar| {
                tracking_key(provider, &connection.connection_id, &calendar.id)
            })
        })
        .collect();

    let now = js_iso8601_timestamp();
    let mut calendar_ids_to_clear: Vec<String> = Vec::new();
    let mut clear_seen: HashSet<String> = HashSet::new();

    for calendar in &existing {
        if calendar.deleted_at.is_some() {
            continue;
        }
        let key = tracking_key(
            &calendar.provider,
            &calendar.connection_id,
            &calendar.tracking_id_calendar,
        );
        let disconnected = !requested.contains(calendar.connection_id.as_str());
        let missing_from_successful_refresh =
            successful.contains(calendar.connection_id.as_str()) && !incoming_keys.contains(&key);

        if disconnected || missing_from_successful_refresh {
            anlg_db_app::soft_delete_calendar(transaction, &now, &calendar.id)
                .await
                .map_err(|error| error.to_string())?;
            if clear_seen.insert(calendar.id.clone()) {
                calendar_ids_to_clear.push(calendar.id.clone());
            }
        } else if !calendar.enabled && clear_seen.insert(calendar.id.clone()) {
            calendar_ids_to_clear.push(calendar.id.clone());
        }
    }

    if !calendar_ids_to_clear.is_empty() {
        anlg_db_app::tombstone_events_for_calendars(transaction, &now, &calendar_ids_to_clear)
            .await
            .map_err(|error| error.to_string())?;
    }

    let mut seen_incoming_keys: HashSet<String> = HashSet::new();
    for connection in &request.successful_connections {
        for calendar in &connection.calendars {
            let key = tracking_key(provider, &connection.connection_id, &calendar.id);
            if !seen_incoming_keys.insert(key.clone()) {
                continue;
            }

            let stored = existing_by_tracking_key.get(&key);
            let calendar_id = stored
                .map(|row| row.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            anlg_db_app::upsert_inventory_calendar(
                transaction,
                &calendar_id,
                &calendar.id,
                &calendar.title,
                provider,
                calendar.source.as_deref().unwrap_or(""),
                calendar.color.as_deref().unwrap_or("#888"),
                &connection.connection_id,
                stored.map(|row| row.created_at.as_str()).unwrap_or(&now),
                &now,
            )
            .await
            .map_err(|error| error.to_string())?;
        }
    }

    Ok(())
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

    fn calendar_list_item(id: &str, title: &str) -> CalendarListItem {
        CalendarListItem {
            provider: CalendarProviderType::Google,
            id: id.to_string(),
            title: title.to_string(),
            source: None,
            color: None,
            is_primary: None,
            can_edit: None,
            raw: "{}".to_string(),
        }
    }

    fn request(
        requested: &[&str],
        successful: Vec<(&str, Vec<CalendarListItem>)>,
    ) -> ApplyCalendarInventoryRequest {
        ApplyCalendarInventoryRequest {
            provider: CalendarProviderType::Google,
            requested_connection_ids: requested.iter().map(|id| id.to_string()).collect(),
            successful_connections: successful
                .into_iter()
                .map(|(connection_id, calendars)| CalendarInventoryConnection {
                    connection_id: connection_id.to_string(),
                    calendars,
                })
                .collect(),
        }
    }

    async fn insert_calendar(db: &Db, id: &str, deleted: bool, enabled: bool) {
        sqlx::query(
            "INSERT INTO calendars (
                id, tracking_id_calendar, name, enabled, provider, source,
                color, connection_id, created_at, updated_at, deleted_at
             ) VALUES (?, 'primary', 'Work', ?, 'google', 'work@example.com',
                '#4285f4', 'conn-work', '2026-01-01T00:00:00.000Z',
                '2026-01-01T00:00:00.000Z', ?)",
        )
        .bind(id)
        .bind(enabled)
        .bind(if deleted {
            Some("2026-06-01T00:00:00.000Z".to_string())
        } else {
            None
        })
        .execute(db.pool())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn soft_deletes_a_disconnected_calendar_and_its_events_atomically() {
        let db = test_db().await;
        insert_calendar(&db, "cal-work", false, true).await;
        sqlx::query(
            "INSERT INTO events (id, tracking_id_event, calendar_id, title, started_at, ended_at)
             VALUES ('event-1', 'track-1', 'cal-work', 'Meeting', '2026-06-01T10:00:00.000Z', '')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        apply_calendar_inventory(db.pool(), request(&[], vec![]))
            .await
            .unwrap();

        let calendar_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM calendars WHERE id = 'cal-work'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let event_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM events WHERE id = 'event-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(calendar_deleted.is_some());
        assert!(event_deleted.is_some());
    }

    #[tokio::test]
    async fn preserves_calendars_when_a_requested_connection_fails_to_refresh() {
        let db = test_db().await;
        insert_calendar(&db, "cal-work", false, true).await;

        apply_calendar_inventory(db.pool(), request(&["conn-work"], vec![]))
            .await
            .unwrap();

        let calendar_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM calendars WHERE id = 'cal-work'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(calendar_deleted.is_none());
    }

    #[tokio::test]
    async fn resurrects_a_calendar_with_its_durable_id_and_disables_it() {
        let db = test_db().await;
        insert_calendar(&db, "cal-work", true, true).await;

        apply_calendar_inventory(
            db.pool(),
            request(
                &["conn-work"],
                vec![(
                    "conn-work",
                    vec![calendar_list_item("primary", "Work restored")],
                )],
            ),
        )
        .await
        .unwrap();

        let row: (String, Option<String>, i64) =
            sqlx::query_as("SELECT name, deleted_at, enabled FROM calendars WHERE id = 'cal-work'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(row.0, "Work restored");
        assert!(row.1.is_none());
        assert_eq!(row.2, 0);
    }

    #[tokio::test]
    async fn inserts_new_calendars_disabled_with_generated_ids() {
        let db = test_db().await;

        apply_calendar_inventory(
            db.pool(),
            request(
                &["conn-work"],
                vec![
                    ("conn-work", vec![calendar_list_item("primary", "Work")]),
                    (
                        "conn-work",
                        vec![calendar_list_item("primary", "Work duplicate")],
                    ),
                ],
            ),
        )
        .await
        .unwrap();

        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT name, enabled FROM calendars WHERE provider = 'google'")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(rows, vec![("Work".to_string(), 0)]);
    }
}
