use crate::AppExt;

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifestEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub main_path: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PluginManifestFile {
    id: String,
    name: String,
    version: String,
    main: String,
}

#[tauri::command]
#[specta::specta]
pub async fn get_onboarding_needed<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    app.get_onboarding_needed().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_onboarding_needed<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: bool,
) -> Result<(), String> {
    app.set_onboarding_needed(v).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_dismissed_toasts<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    app.get_dismissed_toasts()
}

#[tauri::command]
#[specta::specta]
pub async fn set_dismissed_toasts<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: Vec<String>,
) -> Result<(), String> {
    app.set_dismissed_toasts(v)
}

#[tauri::command]
#[specta::specta]
pub async fn get_env<R: tauri::Runtime>(_app: tauri::AppHandle<R>, key: String) -> String {
    std::env::var(&key).unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn show_devtool() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }

    #[cfg(feature = "devtools")]
    {
        return true;
    }

    #[cfg(not(feature = "devtools"))]
    {
        false
    }
}

#[tauri::command]
#[specta::specta]
pub async fn resize_window_for_chat<R: tauri::Runtime>(
    window: tauri::Window<R>,
) -> Result<(), String> {
    let outer_size = window.outer_size().map_err(|e| e.to_string())?;

    let new_size = tauri::PhysicalSize {
        width: outer_size.width + 400,
        height: outer_size.height,
    };
    window
        .set_size(tauri::Size::Physical(new_size))
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn resize_window_for_sidebar<R: tauri::Runtime>(
    window: tauri::Window<R>,
) -> Result<(), String> {
    let outer_size = window.outer_size().map_err(|e| e.to_string())?;

    if outer_size.width < 840 {
        let new_size = tauri::PhysicalSize {
            width: outer_size.width + 280,
            height: outer_size.height,
        };
        window
            .set_size(tauri::Size::Physical(new_size))
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_tinybase_values<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.get_tinybase_values()
}

#[tauri::command]
#[specta::specta]
pub async fn set_tinybase_values<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: String,
) -> Result<(), String> {
    app.set_tinybase_values(v)
}

#[tauri::command]
#[specta::specta]
pub async fn get_pinned_tabs<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.get_pinned_tabs()
}

#[tauri::command]
#[specta::specta]
pub async fn set_pinned_tabs<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: String,
) -> Result<(), String> {
    app.set_pinned_tabs(v)
}

#[tauri::command]
#[specta::specta]
pub async fn get_recently_opened_sessions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.get_recently_opened_sessions()
}

#[tauri::command]
#[specta::specta]
pub async fn set_recently_opened_sessions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: String,
) -> Result<(), String> {
    app.set_recently_opened_sessions(v)
}

#[tauri::command]
#[specta::specta]
pub async fn list_plugins<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<PluginManifestEntry>, String> {
    use tauri_plugin_settings::SettingsPluginExt;

    let base = app.settings().global_base().map_err(|e| e.to_string())?;
    let plugins_dir = base.join("plugins").into_std_path_buf();

    if !plugins_dir.exists() {
        std::fs::create_dir_all(&plugins_dir).map_err(|e| e.to_string())?;
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();

    for entry in std::fs::read_dir(&plugins_dir).map_err(|e| e.to_string())? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if !file_type.is_dir() {
            continue;
        }

        let root = entry.path();
        let manifest_path = root.join("plugin.json");

        if !manifest_path.exists() {
            continue;
        }

        let manifest: PluginManifestFile = match std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PluginManifestFile>(&raw).ok())
        {
            Some(manifest) => manifest,
            None => continue,
        };

        let main_relative = std::path::Path::new(&manifest.main);
        if main_relative.is_absolute()
            || main_relative
                .components()
                .any(|c| c == std::path::Component::ParentDir)
        {
            continue;
        }

        let main_path = root.join(main_relative);
        if !main_path.exists() {
            continue;
        }

        plugins.push(PluginManifestEntry {
            id: manifest.id,
            name: manifest.name,
            version: manifest.version,
            main_path: main_path.to_string_lossy().to_string(),
        });
    }

    plugins.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(plugins)
}
