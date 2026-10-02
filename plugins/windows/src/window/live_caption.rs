use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LiveCaptionPosition {
    TopCenter,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    BottomCenter,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LiveCaptionState {
    pub text: String,
    pub opacity: f64,
    pub width: f64,
    pub line_count: u32,
    pub position: LiveCaptionPosition,
    pub minimized: bool,
}

pub const WINDOW_LABEL: &str = "live-caption";

#[cfg(target_os = "macos")]
mod platform {
    use swift_rs::{Bool, swift};

    use super::LiveCaptionState;
    use crate::Error;

    swift!(fn _live_caption_hide() -> Bool);

    pub fn set_app_handle(_app: tauri::AppHandle<tauri::Wry>) {}

    pub fn current_state() -> Option<LiveCaptionState> {
        None
    }

    pub fn show() -> Result<(), Error> {
        hide()
    }

    pub fn hide() -> Result<(), Error> {
        unsafe {
            _live_caption_hide();
        }
        Ok(())
    }

    pub fn update(_state: LiveCaptionState) -> Result<(), Error> {
        hide()
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::sync::{Mutex, OnceLock};

    use tauri::Manager;

    use super::{LiveCaptionState, WINDOW_LABEL};
    use crate::Error;

    static APP_HANDLE: OnceLock<tauri::AppHandle<tauri::Wry>> = OnceLock::new();
    static LAST_STATE: Mutex<Option<LiveCaptionState>> = Mutex::new(None);

    pub fn set_app_handle(app: tauri::AppHandle<tauri::Wry>) {
        let _ = APP_HANDLE.set(app);
    }

    pub fn current_state() -> Option<LiveCaptionState> {
        LAST_STATE.lock().ok().and_then(|guard| guard.clone())
    }

    fn app() -> Result<&'static tauri::AppHandle<tauri::Wry>, Error> {
        APP_HANDLE
            .get()
            .ok_or_else(|| Error::PanelError("live caption app handle is not ready".to_string()))
    }

    pub fn show() -> Result<(), Error> {
        hide()
    }

    pub fn hide() -> Result<(), Error> {
        if let Ok(app) = app()
            && let Some(window) = app.get_webview_window(WINDOW_LABEL)
        {
            window.hide()?;
        }
        if let Ok(mut state) = LAST_STATE.lock() {
            *state = None;
        }
        Ok(())
    }

    pub fn update(_state: LiveCaptionState) -> Result<(), Error> {
        hide()
    }
}

pub fn set_app_handle(app: tauri::AppHandle<tauri::Wry>) {
    platform::set_app_handle(app);
}

pub fn current_state() -> Option<LiveCaptionState> {
    platform::current_state()
}

pub fn show() -> Result<(), Error> {
    platform::show()
}

pub fn hide() -> Result<(), Error> {
    platform::hide()
}

pub fn update(state: LiveCaptionState) -> Result<(), Error> {
    platform::update(state)
}
