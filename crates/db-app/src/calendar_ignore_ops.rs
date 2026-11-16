//! Hidden calendar events and recurring series (`ignored_events`, `ignored_recurring_series`).

use serde_json::value::RawValue;
use sqlx::SqlitePool;

const LEGACY_MAIN_VALUES_ID: &str = "legacy_main_values_document";
const LEGACY_SETTINGS_ID: &str = "legacy_settings_document";

/// The calendar items a user hid: single events by tracking id, or whole recurring series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoredCalendarItemKind {
    Events,
    Series,
}

impl IgnoredCalendarItemKind {
    fn setting_id(self) -> &'static str {
        match self {
            IgnoredCalendarItemKind::Events => "ignored_events",
            IgnoredCalendarItemKind::Series => "ignored_recurring_series",
        }
    }

    fn key_field(self) -> &'static str {
        match self {
            IgnoredCalendarItemKind::Events => "tracking_id",
            IgnoredCalendarItemKind::Series => "id",
        }
    }
}

/// Adds or removes `item_id` from the kind's ignore list, promoting a legacy list on first
/// write. `now` (a JS ISO timestamp) stamps the entry's `last_seen` and the row.
pub async fn update_ignored_calendar_item(
    pool: &SqlitePool,
    kind: IgnoredCalendarItemKind,
    item_id: &str,
    ignored: bool,
    now: &str,
) -> Result<(), String> {
    let setting_id = kind.setting_id();
    let key_field = kind.key_field();
    let ids = [setting_id, LEGACY_MAIN_VALUES_ID, LEGACY_SETTINGS_ID];

    for _attempt in 0..5 {
        let mut transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|error| error.to_string())?;

        let attempt = async {
            let rows = crate::list_app_setting_rows(&mut transaction, &ids)
                .await
                .map_err(|error| error.to_string())?;
            let direct = rows.iter().find(|row| row.id == setting_id);
            let current = resolve_setting_list(&rows, setting_id);
            let mut entries = current;
            entries.retain(|raw| raw_key_field(raw, key_field).as_deref() != Some(item_id));
            if ignored {
                entries.push(
                    RawValue::from_string(format!(
                        "{{{}:{},\"last_seen\":{}}}",
                        serde_json::to_string(key_field).map_err(|e| e.to_string())?,
                        serde_json::to_string(item_id).map_err(|e| e.to_string())?,
                        serde_json::to_string(now).map_err(|e| e.to_string())?,
                    ))
                    .map_err(|error| error.to_string())?,
                );
            }
            let next_json = format!(
                "[{}]",
                entries
                    .iter()
                    .map(|raw| raw.get().to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );

            let updated = if let Some(direct) = direct {
                crate::update_app_setting_value(
                    &mut transaction,
                    setting_id,
                    &next_json,
                    now,
                    &direct.value_json,
                )
                .await
                .map_err(|error| error.to_string())?
            } else {
                crate::insert_app_setting(&mut transaction, setting_id, &next_json, now)
                    .await
                    .map_err(|error| error.to_string())?
            };
            Ok::<u64, String>(updated)
        }
        .await;

        match attempt {
            Ok(1) => {
                return transaction
                    .commit()
                    .await
                    .map_err(|error| error.to_string());
            }
            Ok(_) => {
                let _ = transaction.rollback().await;
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                return Err(error);
            }
        }
    }

    Err(format!("Setting {setting_id} changed too frequently"))
}

fn raw_key_field(raw: &RawValue, key_field: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw.get()).ok()?;
    value
        .get(key_field)
        .and_then(|field| field.as_str())
        .map(String::from)
}

fn resolve_setting_list(rows: &[crate::AppSettingRow], id: &str) -> Vec<Box<RawValue>> {
    if let Some(direct) = rows.iter().find(|row| row.id == id) {
        return parse_setting_list(Some(&direct.value_json));
    }
    let legacy_main = rows.iter().find(|row| row.id == LEGACY_MAIN_VALUES_ID);
    if let Some(row) = legacy_main
        && has_legacy_setting(&row.value_json, id)
    {
        return parse_legacy_setting_list(&row.value_json, id);
    }
    let legacy_settings = rows.iter().find(|row| row.id == LEGACY_SETTINGS_ID);
    if let Some(row) = legacy_settings {
        return parse_legacy_setting_list(&row.value_json, id);
    }
    Vec::new()
}

fn has_legacy_setting(value: &str, id: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(document) => document
            .as_object()
            .map(|object| object.contains_key(id))
            .unwrap_or(false),
        Err(_) => false,
    }
}

fn parse_legacy_setting_list(value: &str, id: &str) -> Vec<Box<RawValue>> {
    let document: serde_json::Value = match serde_json::from_str(value) {
        Ok(document) => document,
        Err(_) => return Vec::new(),
    };
    let nested = document.get(id).cloned().unwrap_or(serde_json::Value::Null);
    let nested_text = match &nested {
        serde_json::Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "[]".to_string()),
    };
    parse_setting_list(Some(&nested_text))
}

fn parse_setting_list(value: Option<&str>) -> Vec<Box<RawValue>> {
    let Some(value) = value else {
        return Vec::new();
    };
    let parsed: serde_json::Value = match serde_json::from_str(value) {
        Ok(parsed) => parsed,
        Err(_) => return Vec::new(),
    };
    match parsed {
        serde_json::Value::Array(_) => {
            serde_json::from_str::<Vec<Box<RawValue>>>(value).unwrap_or_default()
        }
        serde_json::Value::String(nested) => parse_setting_list(Some(&nested)),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;
    use serde_json::json;

    fn now() -> String {
        // Unique per call, like the JS timestamps callers pass.
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("2026-11-17T00:00:{:02}.{:03}Z", n / 1000 % 60, n % 1000)
    }

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db
    }

    async fn read_setting(db: &Db, id: &str) -> Option<serde_json::Value> {
        let value: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
                .bind(id)
                .fetch_optional(db.pool())
                .await
                .unwrap()
                .flatten();
        value.map(|text| serde_json::from_str(&text).unwrap())
    }

    #[tokio::test]
    async fn ignoring_and_unignoring_events_updates_the_list() {
        let db = test_db().await;

        update_ignored_calendar_item(
            db.pool(),
            IgnoredCalendarItemKind::Events,
            "event-1",
            true,
            &now(),
        )
        .await
        .unwrap();
        let list = read_setting(&db, "ignored_events").await.unwrap();
        assert_eq!(
            list,
            json!([{ "tracking_id": "event-1", "last_seen": list[0]["last_seen"] }])
        );

        update_ignored_calendar_item(
            db.pool(),
            IgnoredCalendarItemKind::Events,
            "event-1",
            false,
            &now(),
        )
        .await
        .unwrap();
        assert_eq!(
            read_setting(&db, "ignored_events").await.unwrap(),
            json!([])
        );
    }

    #[tokio::test]
    async fn ignoring_series_replaces_an_existing_entry() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO app_settings (id, value_json) VALUES ('ignored_recurring_series', ?)",
        )
        .bind(json!([{ "id": "series-1", "last_seen": "old" }]).to_string())
        .execute(db.pool())
        .await
        .unwrap();

        update_ignored_calendar_item(
            db.pool(),
            IgnoredCalendarItemKind::Series,
            "series-1",
            true,
            &now(),
        )
        .await
        .unwrap();

        let list = read_setting(&db, "ignored_recurring_series").await.unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);
        assert_eq!(list[0]["id"], "series-1");
        assert_ne!(list[0]["last_seen"], "old");
    }

    #[tokio::test]
    async fn promotes_the_legacy_settings_document_on_first_mutation() {
        let db = test_db().await;
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(LEGACY_SETTINGS_ID)
            .bind(
                json!({
                    "ignored_events": json!([
                        { "tracking_id": "event-existing", "last_seen": "2026-07-09T00:00:00.000Z" }
                    ]).to_string()
                })
                .to_string(),
            )
            .execute(db.pool())
            .await
            .unwrap();

        update_ignored_calendar_item(
            db.pool(),
            IgnoredCalendarItemKind::Events,
            "event-new",
            true,
            &now(),
        )
        .await
        .unwrap();

        assert_eq!(
            read_setting(&db, "ignored_events").await.unwrap(),
            json!([
                { "tracking_id": "event-existing", "last_seen": "2026-07-09T00:00:00.000Z" },
                { "tracking_id": "event-new", "last_seen": read_setting(&db, "ignored_events").await.unwrap()[1]["last_seen"] }
            ])
        );
    }

    #[tokio::test]
    async fn promotes_ignored_events_recovered_from_legacy_main_values() {
        let db = test_db().await;
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(LEGACY_MAIN_VALUES_ID)
            .bind(
                json!({
                    "ignored_events": json!([
                        { "tracking_id": "event-main", "last_seen": "legacy" }
                    ]).to_string()
                })
                .to_string(),
            )
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(LEGACY_SETTINGS_ID)
            .bind(
                json!({
                    "ignored_events": json!([
                        { "tracking_id": "event-settings", "last_seen": "older" }
                    ]).to_string()
                })
                .to_string(),
            )
            .execute(db.pool())
            .await
            .unwrap();

        update_ignored_calendar_item(
            db.pool(),
            IgnoredCalendarItemKind::Events,
            "event-new",
            true,
            &now(),
        )
        .await
        .unwrap();

        let list = read_setting(&db, "ignored_events").await.unwrap();
        assert_eq!(
            list[0],
            json!({ "tracking_id": "event-main", "last_seen": "legacy" })
        );
        assert_eq!(list[1]["tracking_id"], "event-new");
    }

    #[tokio::test]
    async fn serializes_concurrent_writers_without_dropping_entries() {
        let db = test_db().await;
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES ('ignored_events', ?)")
            .bind(json!([{ "tracking_id": "event-1", "last_seen": "first" }]).to_string())
            .execute(db.pool())
            .await
            .unwrap();

        let pool = db.pool().clone();
        let write = || {
            let pool = pool.clone();
            async move {
                update_ignored_calendar_item(
                    &pool,
                    IgnoredCalendarItemKind::Events,
                    "event-new",
                    true,
                    &now(),
                )
                .await
            }
        };

        let (first, second) = tokio::join!(write(), async {
            let pool2 = db.pool().clone();
            update_ignored_calendar_item(
                &pool2,
                IgnoredCalendarItemKind::Events,
                "event-concurrent",
                true,
                &now(),
            )
            .await
        });
        first.unwrap();
        second.unwrap();

        let list = read_setting(&db, "ignored_events").await.unwrap();
        let ids: Vec<&str> = list
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|entry| entry["tracking_id"].as_str())
            .collect();
        assert_eq!(ids, ["event-1", "event-new", "event-concurrent"]);
    }

    #[tokio::test]
    async fn exhausts_retries_when_the_cas_guard_keeps_missing() {
        let db = test_db().await;
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES ('ignored_events', ?)")
            .bind(json!([{ "tracking_id": "event-1", "last_seen": "first" }]).to_string())
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "CREATE TRIGGER block_ignored_updates BEFORE UPDATE ON app_settings
             WHEN OLD.id = 'ignored_events' BEGIN SELECT RAISE(IGNORE); END",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let error = update_ignored_calendar_item(
            db.pool(),
            IgnoredCalendarItemKind::Events,
            "event-new",
            true,
            &now(),
        )
        .await
        .unwrap_err();

        assert_eq!(error, "Setting ignored_events changed too frequently");
        assert_eq!(
            read_setting(&db, "ignored_events").await.unwrap(),
            json!([{ "tracking_id": "event-1", "last_seen": "first" }])
        );
    }

    #[tokio::test]
    async fn empty_list_overrides_legacy_entries() {
        let db = test_db().await;
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(LEGACY_SETTINGS_ID)
            .bind(
                json!({
                    "ignored_events": json!([
                        { "tracking_id": "event-old", "last_seen": "older" }
                    ]).to_string()
                })
                .to_string(),
            )
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES ('ignored_events', '[]')")
            .execute(db.pool())
            .await
            .unwrap();

        update_ignored_calendar_item(
            db.pool(),
            IgnoredCalendarItemKind::Events,
            "event-new",
            true,
            &now(),
        )
        .await
        .unwrap();

        let list = read_setting(&db, "ignored_events").await.unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);
        assert_eq!(list[0]["tracking_id"], "event-new");
    }
}
