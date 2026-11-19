use std::sync::Arc;

use anlg_db_core::Db;
use anlg_desktop_identity::paths::{
    DB_FILENAME, apply_pending_database_reset, desktop_db_dir,
};

const DEFAULT_CLOUDSYNC_INTERVAL_MS: u64 = 30_000;
const DB_OPEN_LOCK_RETRIES: u32 = 12;
const DB_OPEN_LOCK_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

pub async fn open_desktop_db(identifier: &str) -> Result<Arc<Db>, String> {
    let dir = desktop_db_dir(identifier)
        .ok_or_else(|| "application data directory is unavailable".to_string())?;
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create application data directory: {error}"))?;

    match apply_pending_database_reset(&dir) {
        Ok(Some(backup)) => eprintln!(
            "moved the application database aside for a fresh start: {}",
            backup.display()
        ),
        Ok(None) => {}
        Err(error) => eprintln!("failed to reset the application database: {error}"),
    }
    let db_path = dir.join(DB_FILENAME);

    // During an update relaunch the previous process can hold the database for
    // several seconds while it flushes and exits; retry instead of failing the
    // whole startup on a transient lock.
    let mut attempts = 0u32;
    let db = loop {
        match tauri_plugin_db::open_app_db_unmigrated(Some(&db_path)).await {
            Ok(db) => break db,
            Err(error) if attempts < DB_OPEN_LOCK_RETRIES && is_transient_lock_error(&error) => {
                attempts += 1;
                eprintln!(
                    "application database is locked by another process; \
                     retrying ({attempts}/{DB_OPEN_LOCK_RETRIES}): {error}"
                );
                tokio::time::sleep(DB_OPEN_LOCK_RETRY_DELAY).await;
            }
            Err(error) => {
                return Err(format!("failed to open application database: {error}"));
            }
        }
    };

    Ok(Arc::new(db))
}

pub fn is_transient_lock_error(error: &impl std::fmt::Display) -> bool {
    let message = error.to_string();
    message.contains("database is locked") || message.contains("database table is locked")
}

// Matches MigrateError::SchemaFromNewerApp, which reaches startup as a string
// after crossing the tauri plugin setup boundary.
pub fn is_newer_schema_error(error: &impl std::fmt::Display) -> bool {
    error
        .to_string()
        .contains("created by a newer version of Anarlog")
}
pub fn cloudsync_runtime_config_from_env()
-> Result<Option<anlg_db_core::CloudsyncRuntimeConfig>, String> {
    cloudsync_runtime_config(|key| std::env::var(key).ok())
}

fn cloudsync_runtime_config(
    get: impl Fn(&str) -> Option<String>,
) -> Result<Option<anlg_db_core::CloudsyncRuntimeConfig>, String> {
    let allow_static_auth = get("ANARLOG_CLOUDSYNC_ALLOW_STATIC_AUTH")
        .and_then(nonempty)
        .map(parse_env_flag)
        .transpose()?
        .unwrap_or(false);
    if !allow_static_auth {
        return Ok(None);
    }

    let database_id = get("ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID").and_then(nonempty);
    let api_key = get("ANARLOG_CLOUDSYNC_API_KEY").and_then(nonempty);
    let token = get("ANARLOG_CLOUDSYNC_TOKEN").and_then(nonempty);

    if database_id.is_none() && api_key.is_none() && token.is_none() {
        return Ok(None);
    }

    let database_id = database_id.ok_or_else(|| {
        "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID is required when CloudSync auth is configured"
            .to_string()
    })?;
    let auth = match (api_key, token) {
        (Some(api_key), None) => anlg_db_core::CloudsyncAuth::ApiKey { api_key },
        (None, Some(token)) => anlg_db_core::CloudsyncAuth::Token { token },
        (None, None) => {
            return Err(
                "ANARLOG_CLOUDSYNC_API_KEY or ANARLOG_CLOUDSYNC_TOKEN is required".to_string(),
            );
        }
        (Some(_), Some(_)) => {
            return Err(
                "configure only one of ANARLOG_CLOUDSYNC_API_KEY or ANARLOG_CLOUDSYNC_TOKEN"
                    .to_string(),
            );
        }
    };
    let sync_interval_ms = get("ANARLOG_CLOUDSYNC_INTERVAL_MS")
        .and_then(nonempty)
        .map(|value| {
            value
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    "ANARLOG_CLOUDSYNC_INTERVAL_MS must be a positive integer".to_string()
                })
        })
        .transpose()?
        .unwrap_or(DEFAULT_CLOUDSYNC_INTERVAL_MS);

    Ok(Some(anlg_db_core::CloudsyncRuntimeConfig {
        connection_string: database_id,
        auth,
        tables: anlg_db_app::cloudsync_table_registry().to_vec(),
        sync_interval_ms,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    }))
}

fn nonempty(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_env_flag(value: String) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" => Ok(true),
        "0" | "false" => Ok(false),
        _ => Err("ANARLOG_CLOUDSYNC_ALLOW_STATIC_AUTH must be true, false, 1, or 0".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn startup_error_classifiers_recognize_lock_and_newer_schema_errors() {
        assert!(is_transient_lock_error(
            &"error returned from database: (code: 5) database is locked"
        ));
        assert!(is_transient_lock_error(
            &"error returned from database: (code: 6) database table is locked"
        ));
        assert!(!is_transient_lock_error(
            &"unable to open database file: /tmp/app.db"
        ));

        // Rendered form of MigrateError::SchemaFromNewerApp after crossing the
        // plugin setup boundary as a string.
        assert!(is_newer_schema_error(
            &"plugin db failed: the database was created by a newer version of Anarlog: it requires migration 20260901000000, but this build only includes migrations up to 20260816100100"
        ));
        assert!(!is_newer_schema_error(
            &"unable to open database file: /tmp/app.db"
        ));
    }

    #[test]
    fn cloudsync_environment_config_enables_only_core_tables() {
        let values = HashMap::from([
            ("ANARLOG_CLOUDSYNC_ALLOW_STATIC_AUTH", "true".to_string()),
            (
                "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID",
                "managed-database-id".to_string(),
            ),
            ("ANARLOG_CLOUDSYNC_TOKEN", "token".to_string()),
            ("ANARLOG_CLOUDSYNC_INTERVAL_MS", "15000".to_string()),
        ]);

        let config = cloudsync_runtime_config(|key| values.get(key).cloned())
            .unwrap()
            .unwrap();
        let enabled: Vec<&str> = config
            .tables
            .iter()
            .filter(|table| table.enabled)
            .map(|table| table.table_name.as_str())
            .collect();

        assert_eq!(config.connection_string, "managed-database-id");
        assert_eq!(config.sync_interval_ms, 15_000);
        assert!(matches!(
            config.auth,
            anlg_db_core::CloudsyncAuth::Token { .. }
        ));
        assert_eq!(enabled, vec!["e2ee_records"]);
        assert!(!enabled.contains(&"sessions"));
        assert!(!enabled.contains(&"calendars"));
    }

    #[test]
    fn cloudsync_environment_rejects_multiple_credentials() {
        let values = HashMap::from([
            ("ANARLOG_CLOUDSYNC_ALLOW_STATIC_AUTH", "true".to_string()),
            (
                "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID",
                "managed-database-id".to_string(),
            ),
            ("ANARLOG_CLOUDSYNC_API_KEY", "api-key".to_string()),
            ("ANARLOG_CLOUDSYNC_TOKEN", "token".to_string()),
        ]);

        let error = cloudsync_runtime_config(|key| values.get(key).cloned()).unwrap_err();

        assert!(error.contains("only one"));
    }

    #[test]
    fn cloudsync_stays_inert_without_config_or_static_auth_opt_in() {
        assert!(cloudsync_runtime_config(|_| None).unwrap().is_none());

        let values = HashMap::from([
            (
                "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID",
                "managed-database-id".to_string(),
            ),
            ("ANARLOG_CLOUDSYNC_API_KEY", "api-key".to_string()),
        ]);

        let config = cloudsync_runtime_config(|key| values.get(key).cloned()).unwrap();

        assert!(config.is_none());
    }
}
