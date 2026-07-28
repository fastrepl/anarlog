use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub const DEFAULT_PORT: u16 = 33443;
const SETTINGS_ID: &str = "local_api";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case", default)]
pub struct LocalApiSettings {
    pub enabled: bool,
    pub port: u16,
}

impl Default for LocalApiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
        }
    }
}

pub async fn load(pool: &SqlitePool) -> Result<LocalApiSettings, sqlx::Error> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(SETTINGS_ID)
            .fetch_optional(pool)
            .await?;
    Ok(value
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default())
}

pub async fn store(pool: &SqlitePool, settings: &LocalApiSettings) -> Result<(), sqlx::Error> {
    let value = serde_json::to_string(settings).unwrap_or_else(|_| "null".to_string());
    sqlx::query(
        "INSERT INTO app_settings (id, value_json) VALUES (?, ?) \
         ON CONFLICT(id) DO UPDATE SET \
           value_json = excluded.value_json, \
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(SETTINGS_ID)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}
