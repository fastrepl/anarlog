use crate::{AppWindow, SavedFrames, WindowImpl, WindowsPluginExt, events};

use tauri::Manager;

#[tauri::command]
#[specta::specta]
pub async fn control_set_always_on_top(
    app: tauri::AppHandle<tauri::Wry>,
    always_on_top: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("control")
        .ok_or("control window not found")?;
    window
        .set_always_on_top(always_on_top)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn control_set_opacity(
    app: tauri::AppHandle<tauri::Wry>,
    opacity: f64,
) -> Result<(), String> {
    let window = app
        .get_webview_window("control")
        .ok_or("control window not found")?;

    #[cfg(target_os = "macos")]
    {
        let window_handle = window.clone();
        window
            .run_on_main_thread(move || {
                if let Ok(ns_win) = window_handle.ns_window() {
                    unsafe {
                        let ns_window = &*(ns_win as *mut objc2_app_kit::NSWindow);
                        ns_window.setAlphaValue(opacity);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = opacity;
        let _ = window;
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_show(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    app.windows()
        .show_async(window)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_destroy(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    app.windows().destroy(window).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_navigate(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
    path: String,
) -> Result<(), String> {
    app.windows()
        .navigate(window, path)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_emit_navigate(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
    event: events::Navigate,
) -> Result<(), String> {
    app.windows()
        .emit_navigate(window, event)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Deserialize, specta::Type)]
pub enum Anchor {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
    Center,
}

#[tauri::command]
#[specta::specta]
pub async fn window_set_frame_animated(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
    anchor: Anchor,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let visible_frame = app
        .windows()
        .visible_frame(window.clone())
        .map_err(|e| e.to_string())?;

    if let Some(screen) = visible_frame {
        if matches!(window, AppWindow::Main)
            && let Some(window_handle) = window.get(&app)
        {
            window_handle
                .set_always_on_top(true)
                .map_err(|e| e.to_string())?;
        }

        let margin = 8.0_f64;
        let (x, y) = match anchor {
            Anchor::TopRight => (
                screen.x + screen.w - width - margin,
                screen.y + screen.h - height - margin,
            ),
            Anchor::TopLeft => (screen.x + margin, screen.y + screen.h - height - margin),
            Anchor::BottomRight => (screen.x + screen.w - width - margin, screen.y + margin),
            Anchor::BottomLeft => (screen.x + margin, screen.y + margin),
            Anchor::Center => (
                screen.x + (screen.w - width) / 2.0,
                screen.y + (screen.h - height) / 2.0,
            ),
        };

        let frame = crate::SavedFrame {
            x,
            y,
            w: width,
            h: height,
        };

        app.windows()
            .set_frame_animated(window, frame)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_save_frame(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    let frame = app
        .windows()
        .frame(window.clone())
        .map_err(|e| e.to_string())?;

    if let Some(frame) = frame {
        app.state::<SavedFrames>()
            .0
            .lock()
            .unwrap()
            .insert(window.label(), frame);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_restore_frame_animated(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    let saved = app
        .state::<SavedFrames>()
        .0
        .lock()
        .unwrap()
        .get(&window.label())
        .copied();

    if let Some(saved) = saved {
        app.windows()
            .set_frame_animated(window.clone(), saved)
            .map_err(|e| e.to_string())?;
    }

    if matches!(window, AppWindow::Main)
        && let Some(window_handle) = window.get(&app)
    {
        window_handle
            .set_always_on_top(false)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_is_exists(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<bool, String> {
    let exists = app.windows().is_exists(window).map_err(|e| e.to_string())?;
    Ok(exists)
}
