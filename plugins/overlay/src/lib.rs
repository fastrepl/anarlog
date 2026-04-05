mod commands;
mod ext;

pub use ext::*;

use std::{collections::HashMap, sync::Arc};
use tauri::{AppHandle, Manager, WebviewWindow};
use tokio::{sync::RwLock, task::JoinHandle, time::sleep};

#[cfg(target_os = "macos")]
fn is_window_focused_on_main_thread(app: &AppHandle, window: &WebviewWindow) -> bool {
    let lookup_app = app.clone();
    let label = window.label().to_string();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);

    if app
        .run_on_main_thread(move || {
            let focused = lookup_app
                .get_webview_window(&label)
                .and_then(|window| window.is_focused().ok())
                .unwrap_or(false);
            let _ = tx.send(focused);
        })
        .is_err()
    {
        return false;
    }

    rx.recv().unwrap_or(false)
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, specta::Type, Clone, Copy)]
pub struct OverlayBound {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct FakeWindowBounds(pub Arc<RwLock<HashMap<String, HashMap<String, OverlayBound>>>>);

impl Default for FakeWindowBounds {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
}

pub struct OverlayListenerHandles(pub Arc<RwLock<HashMap<String, JoinHandle<()>>>>);

impl Default for OverlayListenerHandles {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OverlayOptions {
    pub steal_focus: bool,
}

pub async fn abort_overlay_listener(app: &AppHandle, window_label: &str) {
    let handles = app.state::<OverlayListenerHandles>();
    let mut handles_map = handles.0.write().await;
    if let Some(handle) = handles_map.remove(window_label) {
        handle.abort();
    }
}

pub async fn spawn_overlay_listener(
    app: AppHandle,
    window: WebviewWindow,
    options: OverlayOptions,
) {
    let window_label = window.label().to_string();

    abort_overlay_listener(&app, &window_label).await;

    window.set_ignore_cursor_events(true).ok();

    let app_clone = app.clone();
    let handle = tokio::spawn(async move {
        let state = app_clone.state::<FakeWindowBounds>();
        let mut last_ignore_state = true;
        let mut last_focus_state = false;

        loop {
            sleep(std::time::Duration::from_millis(1000 / 20)).await;

            let map = state.0.read().await;

            let Some(windows) = map.get(window.label()) else {
                if !last_ignore_state {
                    window.set_ignore_cursor_events(true).ok();
                    last_ignore_state = true;
                }
                continue;
            };

            if windows.is_empty() {
                if !last_ignore_state {
                    window.set_ignore_cursor_events(true).ok();
                    last_ignore_state = true;
                }
                continue;
            };

            let (Ok(window_position), Ok(mouse_position), Ok(scale_factor)) = (
                window.outer_position(),
                window.cursor_position(),
                window.scale_factor(),
            ) else {
                if !last_ignore_state {
                    if let Err(e) = window.set_ignore_cursor_events(true) {
                        tracing::warn!("Failed to set ignore cursor events: {}", e);
                    }
                    last_ignore_state = true;
                }
                continue;
            };

            let mut ignore = true;

            for (_name, bounds) in windows.iter() {
                let x_min = (window_position.x as f64) + bounds.x * scale_factor;
                let x_max = (window_position.x as f64) + (bounds.x + bounds.width) * scale_factor;
                let y_min = (window_position.y as f64) + bounds.y * scale_factor;
                let y_max = (window_position.y as f64) + (bounds.y + bounds.height) * scale_factor;

                if mouse_position.x >= x_min
                    && mouse_position.x <= x_max
                    && mouse_position.y >= y_min
                    && mouse_position.y <= y_max
                {
                    ignore = false;
                    break;
                }
            }

            if ignore != last_ignore_state {
                if let Err(e) = window.set_ignore_cursor_events(ignore) {
                    tracing::warn!("Failed to set ignore cursor events: {}", e);
                }
                last_ignore_state = ignore;
            }

            if options.steal_focus {
                #[cfg(target_os = "macos")]
                let focused = is_window_focused_on_main_thread(&app_clone, &window);

                #[cfg(not(target_os = "macos"))]
                let focused = window.is_focused().unwrap_or(false);
                if !ignore && !focused {
                    if !last_focus_state && window.set_focus().is_ok() {
                        last_focus_state = true;
                    }
                } else if ignore || focused {
                    last_focus_state = false;
                }
            }
        }
    });

    let handles = app.state::<OverlayListenerHandles>();
    let mut handles_map = handles.0.write().await;
    handles_map.insert(window_label, handle);
}

const PLUGIN_NAME: &str = "overlay";

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::set_fake_window_bounds,
            commands::remove_fake_window,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            app.manage(FakeWindowBounds::default());
            app.manage(OverlayListenerHandles::default());
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }
}
