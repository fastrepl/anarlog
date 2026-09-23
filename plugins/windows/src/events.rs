use std::str::FromStr;

use tauri::Manager;

use crate::{AppWindow, WindowsPluginExt, commands};

// TODO: https://github.com/fastrepl/anarlog/commit/150c8a1 this not worked. webview_window not found.
pub fn on_window_event(window: &tauri::Window<tauri::Wry>, event: &tauri::WindowEvent) {
    let app = window.app_handle();

    if window.label() == crate::window::floating_bar::WINDOW_LABEL {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = crate::window::floating_bar::hide();
        }
        if matches!(event, tauri::WindowEvent::Destroyed) {
            crate::clear_window_state(app, window.label());
        }
        return;
    }

    if window.label() == crate::window::live_caption::WINDOW_LABEL {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = crate::window::live_caption::hide();
        }
        if matches!(event, tauri::WindowEvent::Destroyed) {
            crate::clear_window_state(app, window.label());
        }
        return;
    }

    if matches!(event, tauri::WindowEvent::Destroyed) {
        crate::clear_window_state(app, window.label());
        return;
    }

    #[cfg(not(target_os = "macos"))]
    if let tauri::WindowEvent::Resized(size) = event
        && let Ok(scale) = window.scale_factor()
        && let Some(frame) = app
            .try_state::<crate::PendingPositions>()
            .and_then(|positions| positions.take_if_sized(window.label(), size.to_logical(scale)))
    {
        let _ = window.set_position(tauri::LogicalPosition::new(frame.x, frame.y));
    }

    if matches!(event, tauri::WindowEvent::Focused(false))
        && matches!(window.label().parse::<AppWindow>(), Ok(AppWindow::Main))
        && app
            .try_state::<crate::SavedFrames>()
            .is_some_and(|frames| frames.contains(window.label()))
    {
        let _ = window.set_always_on_top(true);
    }

    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        match window.label().parse::<AppWindow>() {
            Err(e) => tracing::warn!("window_parse_error: {:?}", e),
            Ok(w) => {
                if w == AppWindow::Main {
                    if let Some(saved) = app
                        .try_state::<crate::SavedFrames>()
                        .and_then(|frames| frames.take(window.label()))
                    {
                        api.prevent_close();
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = app.windows().emit_navigate(
                                AppWindow::Main,
                                crate::Navigate {
                                    path: "/app".into(),
                                    search: None,
                                },
                            ) {
                                tracing::warn!(%error, "instruction close navigation failed");
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            if let Err(error) =
                                commands::restore_saved_frame(&app, AppWindow::Main, Some(saved))
                            {
                                tracing::warn!(%error, "instruction close frame restore failed");
                            }
                            if let Err(error) = AppWindow::Main.hide(&app) {
                                tracing::warn!(%error, "instruction close hide failed");
                            }
                        });
                        return;
                    }
                    if window.is_fullscreen().unwrap_or(false) {
                        let _ = window.set_fullscreen(false);
                    }
                    if w.hide(app).is_ok() {
                        api.prevent_close();
                    }
                }
            }
        }
    }
}

#[macro_export]
macro_rules! common_event_derives {
    ($item:item) => {
        #[derive(
            Debug, serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event,
        )]
        $item
    };
}

common_event_derives! {
    pub struct Navigate {
        pub path: String,
        pub search: Option<serde_json::Map<String, serde_json::Value>>,
    }
}

impl FromStr for Navigate {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = url::Url::parse(s)?;

        let path = url.path().to_string();

        let search: Option<serde_json::Map<String, serde_json::Value>> = {
            let pairs: Vec<_> = url.query_pairs().collect();
            if pairs.is_empty() {
                None
            } else {
                let map: serde_json::Map<String, serde_json::Value> = pairs
                    .into_iter()
                    .map(|(k, v)| (k.into_owned(), serde_json::Value::String(v.into_owned())))
                    .collect();
                Some(map)
            }
        };

        Ok(Navigate { path, search })
    }
}

common_event_derives! {
    pub struct WindowDestroyed {
        pub window: AppWindow,
    }
}

common_event_derives! {
    pub struct OpenTab {
        pub tab: crate::TabInput,
    }
}

common_event_derives! {
    pub struct VisibilityEvent {
        pub window: AppWindow,
        pub visible: bool,
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct WebviewHealthCheck {
    pub request_id: String,
}

common_event_derives! {
    pub struct FloatingBarStop {}
}

common_event_derives! {
    pub struct FloatingBarOpenMain {}
}

common_event_derives! {
    pub struct FloatingBarOverlayState {
        pub state: crate::window::floating_bar::FloatingBarState,
    }
}

common_event_derives! {
    pub struct FloatingBarOverlayAmplitude {
        pub amplitude: f64,
    }
}

common_event_derives! {
    pub struct LiveCaptionOverlayState {
        pub state: crate::window::live_caption::LiveCaptionState,
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct FloatingBarSettingsChange {
    pub floating_bar_opacity: Option<f64>,
    pub live_caption_opacity: Option<f64>,
    pub live_caption_width: Option<f64>,
    pub live_caption_line_count: Option<u32>,
    pub live_caption_position: Option<crate::window::live_caption::LiveCaptionPosition>,
    pub live_caption_minimized: Option<bool>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn navigate_from_str() {
        let test_cases = vec![
            (
                "anarlog://anarlog.so/app/new?calendarEventId=123&record=true",
                "/app/new",
                Some(serde_json::json!({ "calendarEventId": "123", "record": "true" })),
            ),
            (
                "hyprnote://hyprnote.com/app/new?record=true",
                "/app/new",
                Some(serde_json::json!({ "record": "true" })),
            ),
        ];

        for (input, expected_path, expected_search) in test_cases {
            let result: Navigate = input.parse().unwrap();

            assert_eq!(result.path, expected_path,);

            match (result.search, expected_search) {
                (Some(actual), Some(expected)) => {
                    let expected_map = expected.as_object().cloned().unwrap();
                    assert_eq!(actual, expected_map);
                }
                (None, None) => {}
                _ => {
                    unreachable!()
                }
            }
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct FloatingBarDictationAction {
    pub session_id: String,
    pub action: DictationPanelAction,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DictationPanelAction {
    Finish,
    Cancel,
    TogglePreview,
}
