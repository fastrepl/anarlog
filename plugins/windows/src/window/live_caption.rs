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
    pub const DEFAULT_WIDTH: f64 = 440.0;
    pub const MAX_WIDTH: f64 = 640.0;
    pub const MIN_LINE_COUNT: u32 = 1;
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
    use swift_rs::{Bool, SRString, swift};

    use super::LiveCaptionState;
    use crate::Error;

    swift!(fn _live_caption_show() -> Bool);
    swift!(fn _live_caption_hide() -> Bool);
    swift!(fn _live_caption_update(json: &SRString) -> Bool);

    pub fn set_app_handle(_app: tauri::AppHandle<tauri::Wry>) {}

    pub fn current_state() -> Option<LiveCaptionState> {
        None
    }

    pub fn show() -> Result<(), Error> {
        unsafe {
            _live_caption_show();
        }
        Ok(())
    }

    pub fn hide() -> Result<(), Error> {
        unsafe {
            _live_caption_hide();
        }
        Ok(())
    }

    pub fn update(state: LiveCaptionState) -> Result<(), Error> {
        let json = serde_json::to_string(&state).map_err(|error| {
            Error::PanelError(format!("failed to serialize live caption state: {error}"))
        })?;
        let json = SRString::from(json.as_str());

        let ok = unsafe { _live_caption_update(&json) };
        if ok {
            Ok(())
        } else {
            Err(Error::PanelError(
                "failed to update native live caption".to_string(),
            ))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::sync::{Mutex, OnceLock};

    use tauri::{
        LogicalPosition, LogicalSize, Manager, Position, Size, WebviewUrl, WebviewWindow,
        WebviewWindowBuilder, window::Color,
    };
    use tauri_specta::Event;

    use super::layout::{self, clamp_to_work_area, origin, window_size};
    use super::{LiveCaptionPosition, LiveCaptionState, WINDOW_LABEL};
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
        let app = app()?;
        let window = ensure_window(app)?;
        let state = current_state();
        if state.as_ref().is_some_and(|value| value.minimized) {
            window.hide()?;
            return Ok(());
        }
        apply_layout(&window, state.as_ref(), true)?;
        window.show()?;
        crate::window::exclude_from_capture(&window);
        Ok(())
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

    pub fn update(state: LiveCaptionState) -> Result<(), Error> {
        let previous_position = current_state().map(|value| value.position);
        if let Ok(mut last) = LAST_STATE.lock() {
            *last = Some(state.clone());
        }
        let app = app()?;
        let _ = crate::events::LiveCaptionOverlayState {
            state: state.clone(),
        }
        .emit(app);
        if state.minimized {
            if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                window.hide()?;
            }
            return Ok(());
        }
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            let force_position = previous_position.is_some_and(|value| value != state.position);
            apply_layout(&window, Some(&state), force_position)?;
            if !window.is_visible().unwrap_or(false) {
                window.show()?;
                crate::window::exclude_from_capture(&window);
            }
        }
        Ok(())
    }

    fn ensure_window(
        app: &tauri::AppHandle<tauri::Wry>,
    ) -> Result<WebviewWindow<tauri::Wry>, Error> {
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            return Ok(window);
        }

        let (width, height) = window_size(layout::DEFAULT_WIDTH, layout::DEFAULT_LINE_COUNT);
        let window = WebviewWindowBuilder::new(
            app,
            WINDOW_LABEL,
            WebviewUrl::App("app/live-caption".into()),
        )
        .title("Anarlog")
        .inner_size(width, height)
        .visible(false)
        .focused(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .content_protected(true)
        .background_color(Color(0, 0, 0, 0))
        .disable_drag_drop_handler()
        .build()?;

        crate::window::exclude_from_capture(&window);

        Ok(window)
    }

    fn apply_layout(
        window: &WebviewWindow<tauri::Wry>,
        state: Option<&LiveCaptionState>,
        force_default_position: bool,
    ) -> Result<(), Error> {
        let width = state
            .map(|value| value.width)
            .unwrap_or(layout::DEFAULT_WIDTH);
        let line_count = state
            .map(|value| value.line_count)
            .unwrap_or(layout::DEFAULT_LINE_COUNT);
        let position = state
            .map(|value| value.position)
            .unwrap_or(LiveCaptionPosition::TopCenter);
        let (width, height) = window_size(width, line_count);
        let next_size = LogicalSize::new(width, height);
        let scale = window.scale_factor()?;
        let current_size = window.outer_size()?.to_logical::<f64>(scale);
        let current_position = window.outer_position()?.to_logical::<f64>(scale);
        let size_changed = (current_size.width - width).abs() >= 0.5
            || (current_size.height - height).abs() >= 0.5;

        if size_changed {
            window.set_size(Size::Logical(next_size))?;
        }

        let (next_x, next_y) =
            if force_default_position || current_position.x == 0.0 && current_position.y == 0.0 {
                default_origin(window, position, width, height)?
            } else if size_changed {
                (current_position.x, current_position.y)
            } else {
                return Ok(());
            };

        let (clamped_x, clamped_y) = clamp_origin(window, next_x, next_y, width, height)?;
        window.set_position(Position::Logical(LogicalPosition::new(
            clamped_x, clamped_y,
        )))?;
        Ok(())
    }

    fn default_origin(
        window: &WebviewWindow<tauri::Wry>,
        position: LiveCaptionPosition,
        width: f64,
        height: f64,
    ) -> Result<(f64, f64), Error> {
        let monitor = window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| window.app_handle().primary_monitor().ok().flatten())
            .ok_or(Error::MonitorNotFound)?;
        let scale = monitor.scale_factor();
        let work_area = monitor.work_area();
        let origin_point = work_area.position.to_logical::<f64>(scale);
        let size = work_area.size.to_logical::<f64>(scale);
        Ok(origin(
            position,
            origin_point.x,
            origin_point.y,
            size.width,
            size.height,
            width,
            height,
        ))
    }

    fn clamp_origin(
        window: &WebviewWindow<tauri::Wry>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(f64, f64), Error> {
        let monitor = window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| window.app_handle().primary_monitor().ok().flatten());
        let Some(monitor) = monitor else {
            return Ok((x, y));
        };
        let scale = monitor.scale_factor();
        let work_area = monitor.work_area();
        let origin_point = work_area.position.to_logical::<f64>(scale);
        let size = work_area.size.to_logical::<f64>(scale);
        Ok(clamp_to_work_area(
            x,
            y,
            width,
            height,
            origin_point.x,
            origin_point.y,
            size.width,
            size.height,
        ))
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
