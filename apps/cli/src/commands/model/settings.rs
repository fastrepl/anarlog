use std::collections::HashMap;
use std::path::{Path, PathBuf};

const DESKTOP_BASE_FOLDERS: &[&str] = &[
    "hyprnote",
    "com.hyprnote.dev",
    "com.hyprnote.staging",
    "com.hyprnote.nightly",
    "com.hyprnote.stable",
    "com.hyprnote.Hyprnote",
];

const CHAR_SETTINGS_PATH: &str = "CHAR_SETTINGS_PATH";
const CHAR_MODELS_BASE: &str = "CHAR_MODELS_BASE";

#[derive(Clone, Debug)]
pub struct DesktopPaths {
    pub global_base: PathBuf,
    pub vault_base: PathBuf,
    pub settings_path: PathBuf,
    pub models_base: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub base_url: Option<String>,
    pub has_api_key: bool,
}

#[derive(Clone, Debug)]
pub struct DesktopSettings {
    pub current_stt_provider: Option<String>,
    pub current_stt_model: Option<String>,
    pub current_llm_provider: Option<String>,
    pub current_llm_model: Option<String>,
    pub stt_providers: HashMap<String, ProviderConfig>,
    pub llm_providers: HashMap<String, ProviderConfig>,
}

pub fn resolve_paths() -> DesktopPaths {
    if let Some(settings_path) = std::env::var_os(CHAR_SETTINGS_PATH).map(PathBuf::from) {
        return build_paths_from_settings_override(settings_path);
    }

    if let Some(paths) = discover_desktop_paths() {
        return with_models_override(paths);
    }

    let data_dir = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let global_base = data_dir.join("char");
    let vault_base = global_base.clone();
    let settings_path = vault_base.join(hypr_storage::vault::SETTINGS_FILENAME);
    let models_base = models_base_override().unwrap_or_else(|| global_base.join("models"));

    DesktopPaths {
        global_base,
        vault_base,
        settings_path,
        models_base,
    }
}

pub fn load_settings(path: &Path) -> Option<DesktopSettings> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let ai = json.get("ai")?.as_object()?;

    let current_stt_provider = get_string(ai.get("current_stt_provider"));
    let current_stt_model = get_string(ai.get("current_stt_model"));
    let current_llm_provider = get_string(ai.get("current_llm_provider"));
    let current_llm_model = get_string(ai.get("current_llm_model"));

    let stt_providers = parse_provider_map(ai.get("stt"));
    let llm_providers = parse_provider_map(ai.get("llm"));

    Some(DesktopSettings {
        current_stt_provider,
        current_stt_model,
        current_llm_provider,
        current_llm_model,
        stt_providers,
        llm_providers,
    })
}

fn build_paths_from_settings_override(settings_path: PathBuf) -> DesktopPaths {
    let vault_base = settings_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);
    let global_base = vault_base.clone();
    let models_base = models_base_override().unwrap_or_else(|| global_base.join("models"));

    DesktopPaths {
        global_base,
        vault_base,
        settings_path,
        models_base,
    }
}

fn with_models_override(mut paths: DesktopPaths) -> DesktopPaths {
    if let Some(override_base) = models_base_override() {
        paths.models_base = override_base;
    }
    paths
}

fn discover_desktop_paths() -> Option<DesktopPaths> {
    let data_dir = dirs::data_dir()?;

    for folder in DESKTOP_BASE_FOLDERS {
        let global_base = data_dir.join(folder);
        let vault_base = hypr_storage::vault::resolve_custom(&global_base, &global_base)
            .unwrap_or(global_base.clone());
        let settings_path = vault_base.join(hypr_storage::vault::SETTINGS_FILENAME);

        if settings_path.is_file() {
            let models_base = global_base.join("models");
            return Some(DesktopPaths {
                global_base,
                vault_base,
                settings_path,
                models_base,
            });
        }
    }

    None
}

fn models_base_override() -> Option<PathBuf> {
    std::env::var_os(CHAR_MODELS_BASE).map(PathBuf::from)
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
        let has_api_key = config_obj
            .get("api_key")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .is_some_and(|v| !v.is_empty());

        out.insert(
            provider_id.clone(),
            ProviderConfig {
                base_url,
                has_api_key,
            },
        );
    }

    out
}
