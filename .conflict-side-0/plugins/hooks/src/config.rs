use tauri_plugin_settings::SettingsPluginExt;

use hypr_hooks::HooksConfig;

use crate::error::{Error, Result};

pub async fn load_config<R: tauri::Runtime>(app: &impl tauri::Manager<R>) -> Result<HooksConfig> {
    let settings = app
        .settings()
        .load()
        .await
        .map_err(|e| Error::ConfigLoad(e.to_string()))?;

    let Some(hooks_value) = settings.get("hooks").cloned() else {
        return Ok(HooksConfig::empty());
    };

    HooksConfig::from_value(hooks_value).map_err(Error::from)
}
