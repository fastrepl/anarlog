use crate::AppExt;

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
    const CHAT_PANEL_EXPANSION_WIDTH: u32 = 400;

    let outer_size = window.outer_size().map_err(|e| e.to_string())?;
    let outer_position = window.outer_position().map_err(|e| e.to_string())?;
    let monitor = window.current_monitor().map_err(|e| e.to_string())?;

    if let Some(monitor) = monitor {
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let window_right = i64::from(outer_position.x) + i64::from(outer_size.width);
        let monitor_right = i64::from(monitor_position.x) + i64::from(monitor_size.width);

        if monitor_right - window_right < i64::from(CHAT_PANEL_EXPANSION_WIDTH) {
            return Ok(());
        }
    }

    let new_size = tauri::PhysicalSize {
        width: outer_size.width + CHAT_PANEL_EXPANSION_WIDTH,
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
pub async fn get_char_v1p1_preview<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    app.get_char_v1p1_preview()
}

#[tauri::command]
#[specta::specta]
pub async fn set_char_v1p1_preview<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: bool,
) -> Result<(), String> {
    app.set_char_v1p1_preview(v)
}
