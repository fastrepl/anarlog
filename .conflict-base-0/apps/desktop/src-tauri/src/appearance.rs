use tauri_plugin_settings::SettingsPluginExt;

#[derive(Clone, Copy)]
pub struct AppAppearanceSettings {
    pub show_app_in_dock: bool,
    pub show_tray_icon: bool,
}

impl Default for AppAppearanceSettings {
    fn default() -> Self {
        Self {
            show_app_in_dock: true,
            show_tray_icon: true,
        }
    }
}

pub fn load_app_appearance_settings<R, M>(manager: &M) -> AppAppearanceSettings
where
    R: tauri::Runtime,
    M: tauri::Manager<R>,
{
    let Ok(path) = manager.settings().settings_path() else {
        return AppAppearanceSettings::default();
    };
    let Ok(content) = std::fs::read_to_string(path.as_std_path()) else {
        return AppAppearanceSettings::default();
    };
    let Ok(settings) = serde_json::from_str::<serde_json::Value>(&content) else {
        return AppAppearanceSettings::default();
    };

    let general = settings.get("general").and_then(|value| value.as_object());

    AppAppearanceSettings {
        show_app_in_dock: general
            .and_then(|section| section.get("show_app_in_dock"))
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        show_tray_icon: general
            .and_then(|section| section.get("show_tray_icon"))
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
    }
}
