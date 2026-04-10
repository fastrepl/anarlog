use std::str::FromStr;

use tauri::Manager;

use crate::AppWindow;

// TODO: https://github.com/fastrepl/char/commit/150c8a1 this not worked. webview_window not found.
pub fn on_window_event(window: &tauri::Window<tauri::Wry>, event: &tauri::WindowEvent) {
    let app = window.app_handle();

    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        match window.label().parse::<AppWindow>() {
            Err(e) => tracing::warn!("window_parse_error: {:?}", e),
            Ok(w) => {
                if w == AppWindow::Main {
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn navigate_from_str() {
        let test_cases = vec![
            (
                "char://char.com/app/new?calendarEventId=123&record=true",
                "/app/new",
                Some(serde_json::json!({ "calendarEventId": "123", "record": "true" })),
            ),
            (
                "char://char.com/app/new?record=true",
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
