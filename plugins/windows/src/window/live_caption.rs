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

pub(crate) mod layout {
    use super::LiveCaptionPosition;

    pub const MIN_WIDTH: f64 = 260.0;
    #[allow(dead_code)]
    pub const DEFAULT_WIDTH: f64 = 440.0;
    pub const MAX_WIDTH: f64 = 640.0;
    pub const MIN_LINE_COUNT: u32 = 1;
    #[allow(dead_code)]
    pub const DEFAULT_LINE_COUNT: u32 = 1;
    pub const MAX_LINE_COUNT: u32 = 4;
    pub const LINE_HEIGHT: f64 = 22.0;
    #[allow(dead_code)]
    pub const HORIZONTAL_PADDING: f64 = 16.0;
    pub const VERTICAL_PADDING: f64 = 10.0;
    pub const FOOTER_HEIGHT: f64 = 32.0;
    pub const FOOTER_SEPARATOR_HEIGHT: f64 = 1.0;
    pub const SCREEN_MARGIN: f64 = 12.0;
    pub const TOP_OFFSET: f64 = 18.0;

    pub fn clamp_width(width: f64) -> f64 {
        width.clamp(MIN_WIDTH, MAX_WIDTH)
    }

    pub fn clamp_line_count(line_count: u32) -> u32 {
        line_count.clamp(MIN_LINE_COUNT, MAX_LINE_COUNT)
    }

    pub fn height(line_count: u32) -> f64 {
        let line_count = clamp_line_count(line_count) as f64;
        VERTICAL_PADDING * 2.0 + LINE_HEIGHT * line_count + FOOTER_SEPARATOR_HEIGHT + FOOTER_HEIGHT
    }

    pub fn window_size(width: f64, line_count: u32) -> (f64, f64) {
        (clamp_width(width), height(line_count))
    }

    pub fn origin(
        position: LiveCaptionPosition,
        work_x: f64,
        work_y: f64,
        work_width: f64,
        work_height: f64,
        window_width: f64,
        window_height: f64,
    ) -> (f64, f64) {
        let top_y = work_y + TOP_OFFSET;
        let bottom_y = work_y + work_height - window_height - SCREEN_MARGIN;
        let center_x = work_x + (work_width - window_width) / 2.0;
        let left_x = work_x + SCREEN_MARGIN;
        let right_x = work_x + work_width - window_width - SCREEN_MARGIN;

        let (x, y) = match position {
            LiveCaptionPosition::TopCenter => (center_x, top_y),
            LiveCaptionPosition::TopLeft => (left_x, top_y),
            LiveCaptionPosition::TopRight => (right_x, top_y),
            LiveCaptionPosition::BottomLeft => (left_x, bottom_y),
            LiveCaptionPosition::BottomRight => (right_x, bottom_y),
            LiveCaptionPosition::BottomCenter => (center_x, bottom_y),
        };

        clamp_to_work_area(
            x,
            y,
            window_width,
            window_height,
            work_x,
            work_y,
            work_width,
            work_height,
        )
    }

    pub fn clamp_to_work_area(
        x: f64,
        y: f64,
        window_width: f64,
        window_height: f64,
        work_x: f64,
        work_y: f64,
        work_width: f64,
        work_height: f64,
    ) -> (f64, f64) {
        let min_x = work_x + SCREEN_MARGIN;
        let min_y = work_y + SCREEN_MARGIN;
        let max_x = work_x + (work_width - window_width - SCREEN_MARGIN).max(0.0);
        let max_y = work_y + (work_height - window_height - SCREEN_MARGIN).max(0.0);
        (
            x.clamp(min_x, max_x.max(min_x)),
            y.clamp(min_y, max_y.max(min_y)),
        )
    }
}

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

#[cfg(test)]
mod tests {
    use super::{LiveCaptionPosition, layout};

    #[test]
    fn sizes_the_caption_window_from_line_count() {
        assert_eq!(layout::window_size(440.0, 1), (440.0, 75.0));
        assert_eq!(layout::window_size(900.0, 4), (640.0, 141.0));
        assert_eq!(layout::window_size(100.0, 0), (260.0, 75.0));
    }

    #[test]
    fn pins_top_center_and_bottom_corners() {
        assert_eq!(
            layout::origin(
                LiveCaptionPosition::TopCenter,
                0.0,
                0.0,
                1920.0,
                1080.0,
                440.0,
                75.0
            ),
            (740.0, 18.0)
        );
        assert_eq!(
            layout::origin(
                LiveCaptionPosition::TopRight,
                0.0,
                0.0,
                1920.0,
                1080.0,
                440.0,
                75.0
            ),
            (1468.0, 18.0)
        );
        assert_eq!(
            layout::origin(
                LiveCaptionPosition::BottomLeft,
                0.0,
                0.0,
                1920.0,
                1080.0,
                440.0,
                75.0
            ),
            (12.0, 993.0)
        );
    }
}
