mod analytics;
mod sentry;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anlg_desktop_identity::{Channel, store_json::StoreJson};
use serde_json::{Map, Value, json};

use crate::product::settings::{self, Snapshot};

static CRASH_REPORTING_ENABLED: AtomicBool = AtomicBool::new(false);
static TELEMETRY_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_telemetry_enabled(enabled: bool) {
    TELEMETRY_ENABLED.store(enabled, Ordering::SeqCst);
    if enabled {
        analytics::sync_identity();
        analytics::sync_settings_properties_if_pending();
    }
}

pub fn set_crash_reporting_enabled(enabled: bool) {
    CRASH_REPORTING_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn telemetry_enabled() -> bool {
    TELEMETRY_ENABLED.load(Ordering::SeqCst)
}

pub fn crash_reporting_enabled() -> bool {
    CRASH_REPORTING_ENABLED.load(Ordering::SeqCst)
}

pub fn consent_from_settings(snapshot: &Snapshot) -> (bool, bool) {
    let telemetry = snapshot
        .get("telemetry_consent")
        .and_then(|setting| setting.value.as_bool())
        .unwrap_or(true);
    let crash = snapshot
        .get("crash_reporting_consent")
        .filter(|setting| setting.source != "Default")
        .and_then(|setting| setting.value.as_bool())
        .unwrap_or(telemetry);
    (telemetry, crash)
}

/// Reads managed settings (fail-closed on any validation/read error) and existing local
/// SQLite consent flags before `init_sentry` / `init_analytics`, without modifying or creating DB files.
pub fn load_startup_consent(database: &Path) -> (bool, bool) {
    let managed = match database
        .parent()
        .map(|dir| anlg_db_app::read_managed_settings(&dir.join("managed_settings.json")))
    {
        Some(Ok(settings)) => settings,
        Some(Err(error)) => {
            tracing::warn!(%error, "failing closed: unreadable or invalid managed_settings.json");
            return (false, false);
        }
        None => return (false, false),
    };

    let mut rows = if database.is_file() {
        let Some(rows) = read_database_consent_rows(database) else {
            return (false, false);
        };
        rows
    } else {
        Vec::new()
    };

    for key in ["telemetry_consent", "crash_reporting_consent"] {
        if let Some(value) = managed.get(key) {
            rows.retain(|row| row["id"] != key);
            rows.push(json!({
                "id": key,
                "value_json": value.to_string(),
                "oversized": 0,
                "source_rank": 0,
            }));
        }
    }

    if !consent_rows_valid(&rows) {
        return (false, false);
    }

    settings::decode(&rows)
        .map(|snapshot| consent_from_settings(&snapshot))
        .unwrap_or((false, false))
}

fn consent_rows_valid(rows: &[Value]) -> bool {
    for row in rows {
        let Some(id) = row["id"].as_str() else {
            return false;
        };
        let Some(value_json) = row["value_json"].as_str() else {
            return false;
        };
        let Ok(parsed) = serde_json::from_str::<Value>(value_json) else {
            return false;
        };
        match id {
            "telemetry_consent" | "crash_reporting_consent" if !parsed.is_boolean() => {
                return false;
            }
            "legacy_settings_document" => {
                let Some(obj) = parsed.as_object() else {
                    return false;
                };
                if let Some(legacy_consent) =
                    obj.get("general").and_then(|g| g.get("telemetry_consent"))
                    && !legacy_consent.is_null()
                    && !legacy_consent.is_boolean()
                {
                    return false;
                }
            }
            "legacy_main_values_document" if !parsed.is_object() => return false,
            _ => {}
        }
    }
    true
}

fn read_database_consent_rows(database: &Path) -> Option<Vec<Value>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(async move {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(database)
            .read_only(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .ok()?;
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'app_settings'",
        )
        .fetch_optional(&pool)
        .await
        .ok()?;
        if table_exists.is_none() {
            return Some(Vec::new());
        }
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, value_json FROM app_settings WHERE id IN ('telemetry_consent', 'crash_reporting_consent', 'legacy_settings_document', 'legacy_main_values_document')",
        )
        .fetch_all(&pool)
        .await
        .ok()?;
        Some(
            rows.into_iter()
                .map(|(id, value_json)| {
                    json!({
                        "id": id,
                        "value_json": value_json,
                        "oversized": 0,
                        "source_rank": 0,
                    })
                })
                .collect(),
        )
    })
}

pub fn init_sentry(channel: Option<Channel>) -> Option<::sentry::ClientInitGuard> {
    sentry::init(channel)
}

pub fn init_analytics(channel: Option<Channel>, shipping_store: Option<Arc<StoreJson>>) {
    analytics::init(channel, shipping_store);
}

pub fn sentry_tracing_layer<S>() -> impl tracing_subscriber::Layer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    sentry::tracing_layer()
}

pub fn redacting_stderr() -> sentry::RedactingStderr {
    sentry::redacting_stderr()
}

pub fn capture_operational_error(
    operation: &'static str,
    stage: Option<&'static str>,
    error: &str,
    provider: Option<&str>,
    status_code: Option<u16>,
) {
    sentry::capture_operational_error(operation, stage, error, provider, status_code);
}

pub fn track(event: impl Into<String>, props: &[(&str, Value)]) {
    analytics::track(event, props);
}

pub fn set_properties(set: Map<String, Value>, set_once: Map<String, Value>) {
    analytics::set_properties(set, set_once);
}

pub fn identify(user_id: &str, plan: &str, trial_end: Option<i64>, created_at: Option<&str>) {
    analytics::identify(user_id, plan, trial_end, created_at);
}

pub fn sync_settings_properties(snapshot: &Snapshot) {
    analytics::sync_settings_properties(snapshot);
}

pub fn track_first_open_if_needed(shipping_store: Option<&Arc<StoreJson>>) {
    analytics::track_first_open_if_needed(shipping_store);
}

pub fn clear_identity() {
    analytics::clear_identity();
}

pub fn flush(timeout: Duration) -> bool {
    analytics::flush(timeout)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn row(id: &str, value_json: &str) -> Value {
        json!({
            "id": id,
            "value_json": value_json,
            "oversized": 0,
            "source_rank": 0,
        })
    }

    #[test]
    fn consent_from_settings_honors_legacy_and_direct_overrides() {
        let defaults = crate::product::settings::decode(&[]).unwrap();
        assert_eq!(consent_from_settings(&defaults), (true, true));

        let legacy_opt_out = crate::product::settings::decode(&[row(
            "legacy_settings_document",
            r#"{"general":{"telemetry_consent":false}}"#,
        )])
        .unwrap();
        assert_eq!(consent_from_settings(&legacy_opt_out), (false, false));

        let legacy_split = crate::product::settings::decode(&[
            row(
                "legacy_settings_document",
                r#"{"general":{"telemetry_consent":false}}"#,
            ),
            row("crash_reporting_consent", "true"),
        ])
        .unwrap();
        assert_eq!(consent_from_settings(&legacy_split), (false, true));

        let direct_override = crate::product::settings::decode(&[
            row(
                "legacy_settings_document",
                r#"{"general":{"telemetry_consent":false}}"#,
            ),
            row("telemetry_consent", "true"),
            row("crash_reporting_consent", "false"),
        ])
        .unwrap();
        assert_eq!(consent_from_settings(&direct_override), (true, false));
    }

    #[test]
    fn startup_consent_reads_database_and_fails_closed_on_managed_settings() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        assert_eq!(load_startup_consent(&db_path), (true, true));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let pool = rt.block_on(async {
            let options = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true);
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .unwrap();
            sqlx::query("CREATE TABLE app_settings (id TEXT PRIMARY KEY, value_json TEXT NOT NULL)")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query(
                "INSERT OR REPLACE INTO app_settings (id, value_json) VALUES ('telemetry_consent', 'false'), ('crash_reporting_consent', 'true')",
            )
            .execute(&pool)
            .await
            .unwrap();
            pool
        });
        assert_eq!(load_startup_consent(&db_path), (false, true));

        std::fs::write(
            dir.path().join("managed_settings.json"),
            br#"{"schema_version":1,"settings":{"telemetry_consent":false,"crash_reporting_consent":false}}"#,
        )
        .unwrap();
        assert_eq!(load_startup_consent(&db_path), (false, false));

        rt.block_on(async {
            sqlx::query(
                "INSERT OR REPLACE INTO app_settings (id, value_json) VALUES ('telemetry_consent', 'true'), ('crash_reporting_consent', 'true')",
            )
            .execute(&pool)
            .await
            .unwrap();
        });
        assert_eq!(load_startup_consent(&db_path), (false, false));

        std::fs::write(dir.path().join("managed_settings.json"), b"not-json").unwrap();
        assert_eq!(load_startup_consent(&db_path), (false, false));

        std::fs::remove_file(dir.path().join("managed_settings.json")).unwrap();
        rt.block_on(async {
            sqlx::query(
                "INSERT OR REPLACE INTO app_settings (id, value_json) VALUES ('telemetry_consent', '\"invalid\"')",
            )
            .execute(&pool)
            .await
            .unwrap();
        });
        assert_eq!(load_startup_consent(&db_path), (false, false));
    }
}
