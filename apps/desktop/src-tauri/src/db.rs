use std::sync::Arc;

use hypr_db_core::Db;

const DB_FILENAME: &str = "app.db";
const DEFAULT_CLOUDSYNC_INTERVAL_MS: u64 = 30_000;

pub async fn open_desktop_db(identifier: &str) -> Arc<Db> {
    let db_path = desktop_db_dir(identifier).map(|dir| {
        std::fs::create_dir_all(&dir).expect("failed to create app data dir");
        dir.join(DB_FILENAME)
    });

    let db = tauri_plugin_db::open_app_db(db_path.as_deref())
        .await
        .expect("failed to open app database");

    Arc::new(db)
}

pub fn cloudsync_runtime_config_from_env()
-> Result<Option<hypr_db_core::CloudsyncRuntimeConfig>, String> {
    cloudsync_runtime_config(|key| std::env::var(key).ok())
}

fn cloudsync_runtime_config(
    get: impl Fn(&str) -> Option<String>,
) -> Result<Option<hypr_db_core::CloudsyncRuntimeConfig>, String> {
    let allow_static_auth = get("ANARLOG_CLOUDSYNC_ALLOW_STATIC_AUTH")
        .and_then(nonempty)
        .map(parse_env_flag)
        .transpose()?
        .unwrap_or(false);
    if !allow_static_auth {
        return Ok(None);
    }

    let database_id = get("ANARLOG_CLOUDSYNC_DATABASE_ID").and_then(nonempty);
    let api_key = get("ANARLOG_CLOUDSYNC_API_KEY").and_then(nonempty);
    let token = get("ANARLOG_CLOUDSYNC_TOKEN").and_then(nonempty);

    if database_id.is_none() && api_key.is_none() && token.is_none() {
        return Ok(None);
    }

    let database_id = database_id.ok_or_else(|| {
        "ANARLOG_CLOUDSYNC_DATABASE_ID is required when CloudSync auth is configured".to_string()
    })?;
    let auth = match (api_key, token) {
        (Some(api_key), None) => hypr_db_core::CloudsyncAuth::ApiKey { api_key },
        (None, Some(token)) => hypr_db_core::CloudsyncAuth::Token { token },
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

    Ok(Some(hypr_db_core::CloudsyncRuntimeConfig {
        connection_string: database_id,
        auth,
        tables: hypr_db_app::cloudsync_table_registry().to_vec(),
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

fn desktop_db_dir(identifier: &str) -> Option<std::path::PathBuf> {
    let data_dir = dirs::data_dir().expect("data_dir must be available");
    let default_dir =
        hypr_storage::global::compute_default_base(identifier).expect("data_dir must be available");
    let identifier_dir = data_dir.join(identifier);

    if identifier_dir.join(DB_FILENAME).is_file() && !default_dir.join(DB_FILENAME).is_file() {
        Some(identifier_dir)
    } else {
        Some(default_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn dev_uses_an_isolated_persistent_database() {
        let db_dir = desktop_db_dir("com.hyprnote.dev").unwrap();

        assert!(db_dir.ends_with("com.hyprnote.dev"));
    }

    #[test]
    fn cloudsync_is_inert_without_environment_config() {
        let config = cloudsync_runtime_config(|_| None).unwrap();

        assert!(config.is_none());
    }

    #[test]
    fn cloudsync_environment_config_enables_only_core_tables() {
        let values = HashMap::from([
            ("ANARLOG_CLOUDSYNC_ALLOW_STATIC_AUTH", "true".to_string()),
            (
                "ANARLOG_CLOUDSYNC_DATABASE_ID",
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
            hypr_db_core::CloudsyncAuth::Token { .. }
        ));
        assert_eq!(enabled.len(), 8);
        assert!(enabled.contains(&"sessions"));
        assert!(!enabled.contains(&"calendars"));
    }

    #[test]
    fn cloudsync_environment_rejects_multiple_credentials() {
        let values = HashMap::from([
            ("ANARLOG_CLOUDSYNC_ALLOW_STATIC_AUTH", "true".to_string()),
            (
                "ANARLOG_CLOUDSYNC_DATABASE_ID",
                "managed-database-id".to_string(),
            ),
            ("ANARLOG_CLOUDSYNC_API_KEY", "api-key".to_string()),
            ("ANARLOG_CLOUDSYNC_TOKEN", "token".to_string()),
        ]);

        let error = cloudsync_runtime_config(|key| values.get(key).cloned()).unwrap_err();

        assert!(error.contains("only one"));
    }

    #[test]
    fn cloudsync_static_auth_requires_explicit_opt_in() {
        let values = HashMap::from([
            (
                "ANARLOG_CLOUDSYNC_DATABASE_ID",
                "managed-database-id".to_string(),
            ),
            ("ANARLOG_CLOUDSYNC_API_KEY", "api-key".to_string()),
        ]);

        let config = cloudsync_runtime_config(|key| values.get(key).cloned()).unwrap();

        assert!(config.is_none());
    }
}
