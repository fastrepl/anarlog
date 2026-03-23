use std::collections::HashMap;
use std::path::Path;

use sqlx::SqlitePool;

#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub current_stt_provider: Option<String>,
    pub current_stt_model: Option<String>,
    pub stt_providers: HashMap<String, ProviderConfig>,
}

pub async fn load_settings(pool: &SqlitePool) -> Option<Settings> {
    let all = hypr_db_app::load_all_settings(pool).await.ok()?;
    let setting_map: HashMap<String, String> = all.into_iter().collect();

    let current_stt_provider = setting_map
        .get("current_stt_provider")
        .filter(|v| !v.is_empty())
        .cloned();
    let current_stt_model = setting_map
        .get("current_stt_model")
        .filter(|v| !v.is_empty())
        .cloned();

    let stt_connections = hypr_db_app::list_connections(pool, "stt").await.ok()?;
    let stt_providers = connections_to_provider_map(stt_connections);

    if current_stt_provider.is_none() && stt_providers.is_empty() {
        return None;
    }

    Some(Settings {
        current_stt_provider,
        current_stt_model,
        stt_providers,
    })
}

fn connections_to_provider_map(
    connections: Vec<hypr_db_app::ConnectionRow>,
) -> HashMap<String, ProviderConfig> {
    connections
        .into_iter()
        .map(|c| {
            let base_url = if c.base_url.is_empty() {
                None
            } else {
                Some(c.base_url)
            };
            let api_key = if c.api_key.is_empty() {
                None
            } else {
                Some(c.api_key)
            };
            (c.provider_id, ProviderConfig { base_url, api_key })
        })
        .collect()
}

pub async fn migrate_json_settings_to_db(pool: &SqlitePool, base_path: &Path) {
    let has_settings = hypr_db_app::load_all_settings(pool)
        .await
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let has_connections = hypr_db_app::list_connections(pool, "stt")
        .await
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    if has_settings || has_connections {
        return;
    }

    let json_path = base_path.join("settings.json");
    let Some(settings) = load_settings_from_json(&json_path) else {
        return;
    };

    if let Some(ref v) = settings.current_stt_provider {
        let _ = hypr_db_app::set_setting(pool, "current_stt_provider", v).await;
    }
    if let Some(ref v) = settings.current_stt_model {
        let _ = hypr_db_app::set_setting(pool, "current_stt_model", v).await;
    }

    for (provider_id, config) in &settings.stt_providers {
        let _ = hypr_db_app::upsert_connection(
            pool,
            "stt",
            provider_id,
            config.base_url.as_deref().unwrap_or(""),
            config.api_key.as_deref().unwrap_or(""),
        )
        .await;
    }
}

fn load_settings_from_json(path: &Path) -> Option<Settings> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let ai = json.get("ai")?.as_object()?;

    let current_stt_provider = get_string(ai.get("current_stt_provider"));
    let current_stt_model = get_string(ai.get("current_stt_model"));
    let stt_providers = parse_provider_map(ai.get("stt"));

    Some(Settings {
        current_stt_provider,
        current_stt_model,
        stt_providers,
    })
}

fn get_string(value: Option<&serde_json::Value>) -> Option<String> {
    value?.as_str().map(ToString::to_string)
}

fn parse_provider_map(value: Option<&serde_json::Value>) -> HashMap<String, ProviderConfig> {
    let mut out = HashMap::new();
    let Some(obj) = value.and_then(serde_json::Value::as_object) else {
        return out;
    };

    for (provider_id, config) in obj {
        let Some(config_obj) = config.as_object() else {
            continue;
        };

        let base_url = config_obj
            .get("base_url")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string);
        let api_key = config_obj
            .get("api_key")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string);
        out.insert(provider_id.clone(), ProviderConfig { base_url, api_key });
    }

    out
}
