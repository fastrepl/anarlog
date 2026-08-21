use serde::{Deserialize, Serialize};

use crate::Error;
use crate::window::live_caption::LiveCaptionPosition;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FloatingBarStatus {
    Recording,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FloatingBarColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FloatingTranscriptBubble {
    pub id: String,
    pub speaker_label: String,
    pub text: String,
    pub is_self: bool,
    pub is_final: bool,
    pub start_ms: f64,
    pub end_ms: f64,
    pub overlaps_previous: bool,
    pub overlaps_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FloatingBarState {
    pub amplitude: f64,
    pub title: String,
    pub status: FloatingBarStatus,
    pub color_scheme: FloatingBarColorScheme,
    pub opacity: f64,
    pub live_caption_opacity: f64,
    pub live_caption_width: f64,
    pub live_caption_line_count: u32,
    pub live_caption_position: LiveCaptionPosition,
    pub live_caption_minimized: bool,
    pub live_caption_toggle_visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_bubbles: Option<Vec<FloatingTranscriptBubble>>,
}

pub const WINDOW_LABEL: &str = "floating-bar";

pub(crate) mod layout {
    use super::FloatingBarState;

    pub const INSET: f64 = 4.0;
    pub const SCREEN_MARGIN: f64 = 8.0;
    pub const COMPACT_HEIGHT: f64 = 38.0;
    pub const COMPACT_STOP_WIDTH: f64 = 62.0;
    pub const COMPACT_SOLO_STOP_WIDTH: f64 = 68.0;
    pub const COMPACT_ICON_SIZE: f64 = 30.0;
    pub const COMPACT_GAP: f64 = 3.0;
    pub const COMPACT_HORIZONTAL_PADDING: f64 = 4.0;
    pub const EXPANDED_WIDTH: f64 = 360.0;
    pub const EXPANDED_HEIGHT: f64 = 430.0;
    pub const HOVER_HANDLE_TOP_PADDING: f64 = 7.0;
    pub const HOVER_HANDLE_HEIGHT: f64 = 12.0;
    pub const HOVER_HANDLE_GAP: f64 = 2.0;
    pub const HOVER_HANDLE_RESERVED_HEIGHT: f64 =
        HOVER_HANDLE_TOP_PADDING + HOVER_HANDLE_HEIGHT + HOVER_HANDLE_GAP;

    pub fn is_expanded(state: &FloatingBarState) -> bool {
        state.live_caption_toggle_visible && !state.live_caption_minimized
    }

    pub fn compact_controls_width(shows_expand: bool) -> f64 {
        if shows_expand {
            COMPACT_STOP_WIDTH + COMPACT_GAP + COMPACT_ICON_SIZE
        } else {
            COMPACT_SOLO_STOP_WIDTH
        }
    }

    pub fn compact_width(shows_expand: bool) -> f64 {
        compact_controls_width(shows_expand) + COMPACT_HORIZONTAL_PADDING * 2.0
    }

    pub fn container_size(is_expanded: bool, shows_expand: bool) -> (f64, f64) {
        if is_expanded {
            (
                EXPANDED_WIDTH + INSET * 2.0,
                EXPANDED_HEIGHT + HOVER_HANDLE_RESERVED_HEIGHT + INSET * 2.0,
            )
        } else {
            (
                compact_width(shows_expand) + INSET * 2.0,
                COMPACT_HEIGHT + HOVER_HANDLE_RESERVED_HEIGHT + INSET * 2.0,
            )
        }
    }

    pub fn top_right_origin(
        work_x: f64,
        work_y: f64,
        work_width: f64,
        _work_height: f64,
        window_width: f64,
        _window_height: f64,
    ) -> (f64, f64) {
        (
            work_x + work_width - window_width - SCREEN_MARGIN,
            work_y + SCREEN_MARGIN,
        )
    }

    pub fn resize_keep_top_right(
        x: f64,
        y: f64,
        current_width: f64,
        _current_height: f64,
        next_width: f64,
        _next_height: f64,
    ) -> (f64, f64) {
        (x + current_width - next_width, y)
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
        let max_x = work_x + (work_width - window_width).max(0.0);
        let max_y = work_y + (work_height - window_height).max(0.0);
        (x.clamp(work_x, max_x), y.clamp(work_y, max_y))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use std::sync::OnceLock;

    use swift_rs::{Bool, SRString, swift};
    use tauri_specta::Event;

    use super::FloatingBarState;
    use crate::Error;

    swift!(fn _floating_bar_show() -> Bool);
    swift!(fn _floating_bar_hide() -> Bool);
    swift!(fn _floating_bar_update(json: &SRString) -> Bool);
    swift!(fn _floating_bar_update_amplitude(amplitude: f64) -> Bool);

    static APP_HANDLE: OnceLock<tauri::AppHandle<tauri::Wry>> = OnceLock::new();

    pub fn set_app_handle(app: tauri::AppHandle<tauri::Wry>) {
        let _ = APP_HANDLE.set(app);
    }

    pub fn show() -> Result<(), Error> {
        unsafe {
            _floating_bar_show();
        }
        Ok(())
    }

    pub fn hide() -> Result<(), Error> {
        unsafe {
            _floating_bar_hide();
        }
        Ok(())
    }

    pub fn update(state: FloatingBarState) -> Result<(), Error> {
        let json = serde_json::to_string(&state).map_err(|error| {
            Error::PanelError(format!("failed to serialize floating bar state: {error}"))
        })?;
        let ok = swift_rs::autoreleasepool!({
            let json = SRString::from(json.as_str());
            unsafe { _floating_bar_update(&json) }
        });
        if ok {
            Ok(())
        } else {
            Err(Error::PanelError(
                "failed to update native floating bar".to_string(),
            ))
        }
    }

    pub fn update_amplitude(amplitude: f64) -> Result<(), Error> {
        let ok = unsafe { _floating_bar_update_amplitude(amplitude) };
        if ok {
            Ok(())
        } else {
            Err(Error::PanelError(
                "failed to update native floating bar amplitude".to_string(),
            ))
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rust_on_floating_bar_stop() {
        if let Some(app) = APP_HANDLE.get() {
            let _ = crate::events::FloatingBarStop {}.emit(app);
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rust_on_floating_bar_open_main() {
        if let Some(app) = APP_HANDLE.get() {
            let _ = crate::events::FloatingBarOpenMain {}.emit(app);
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rust_on_floating_bar_settings_change(settings_ptr: *const c_char) {
        if settings_ptr.is_null() {
            return;
        }

        let Ok(settings_json) = (unsafe { CStr::from_ptr(settings_ptr) }).to_str() else {
            return;
        };

        let Ok(settings) =
            serde_json::from_str::<crate::events::FloatingBarSettingsChange>(settings_json)
        else {
            return;
        };

        if let Some(app) = APP_HANDLE.get() {
            let _ = settings.emit(app);
        }
    }

    pub fn current_state() -> Option<FloatingBarState> {
        None
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

    use super::layout::{
        clamp_to_work_area, container_size, is_expanded, resize_keep_top_right, top_right_origin,
    };
    use super::{FloatingBarState, WINDOW_LABEL};
    use crate::Error;

    static APP_HANDLE: OnceLock<tauri::AppHandle<tauri::Wry>> = OnceLock::new();
    static LAST_STATE: Mutex<Option<FloatingBarState>> = Mutex::new(None);

    pub fn set_app_handle(app: tauri::AppHandle<tauri::Wry>) {
        let _ = APP_HANDLE.set(app);
    }

    pub fn current_state() -> Option<FloatingBarState> {
        LAST_STATE.lock().ok().and_then(|guard| guard.clone())
    }

    fn app() -> Result<&'static tauri::AppHandle<tauri::Wry>, Error> {
        APP_HANDLE
            .get()
            .ok_or_else(|| Error::PanelError("floating bar app handle is not ready".to_string()))
    }

    pub fn show() -> Result<(), Error> {
        let app = app()?;
        let window = ensure_window(app)?;
        let state = current_state();
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

    pub fn update(state: FloatingBarState) -> Result<(), Error> {
        if let Ok(mut last) = LAST_STATE.lock() {
            *last = Some(state.clone());
        }
        let app = app()?;
        let _ = crate::events::FloatingBarOverlayState {
            state: state.clone(),
        }
        .emit(app);
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            apply_layout(&window, Some(&state), false)?;
        }
        Ok(())
    }

    pub fn update_amplitude(amplitude: f64) -> Result<(), Error> {
        let amplitude = amplitude.clamp(0.0, 1.0);
        if let Ok(mut last) = LAST_STATE.lock()
            && let Some(state) = last.as_mut()
        {
            state.amplitude = amplitude;
        }
        if let Ok(app) = app() {
            let _ = crate::events::FloatingBarOverlayAmplitude { amplitude }.emit(app);
        }
        Ok(())
    }

    fn ensure_window(
        app: &tauri::AppHandle<tauri::Wry>,
    ) -> Result<WebviewWindow<tauri::Wry>, Error> {
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            return Ok(window);
        }

        let (width, height) = container_size(false, false);
        let window = WebviewWindowBuilder::new(
            app,
            WINDOW_LABEL,
            WebviewUrl::App("app/floating-bar".into()),
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
        state: Option<&FloatingBarState>,
        force_default_position: bool,
    ) -> Result<(), Error> {
        let is_expanded = state.is_some_and(is_expanded);
        let shows_expand = state.is_some_and(|value| value.live_caption_toggle_visible);
        let (width, height) = container_size(is_expanded, shows_expand);
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
                default_origin(window, width, height)?
            } else if size_changed {
                resize_keep_top_right(
                    current_position.x,
                    current_position.y,
                    current_size.width,
                    current_size.height,
                    width,
                    height,
                )
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
        let origin = work_area.position.to_logical::<f64>(scale);
        let size = work_area.size.to_logical::<f64>(scale);
        Ok(top_right_origin(
            origin.x,
            origin.y,
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
        let origin = work_area.position.to_logical::<f64>(scale);
        let size = work_area.size.to_logical::<f64>(scale);
        Ok(clamp_to_work_area(
            x,
            y,
            width,
            height,
            origin.x,
            origin.y,
            size.width,
            size.height,
        ))
    }
}

pub fn set_app_handle(app: tauri::AppHandle<tauri::Wry>) {
    platform::set_app_handle(app);
}

pub fn current_state() -> Option<FloatingBarState> {
    platform::current_state()
}

pub fn show() -> Result<(), Error> {
    platform::show()
}

pub fn hide() -> Result<(), Error> {
    platform::hide()
}

pub fn update(state: FloatingBarState) -> Result<(), Error> {
    platform::update(state)
}

pub fn update_amplitude(amplitude: f64) -> Result<(), Error> {
    platform::update_amplitude(amplitude)
}

#[cfg(test)]
mod tests {
    use super::layout;

    #[test]
    fn sizes_the_compact_and_expanded_windows() {
        assert_eq!(layout::container_size(false, false), (84.0, 67.0));
        assert_eq!(layout::container_size(false, true), (111.0, 67.0));
        assert_eq!(layout::container_size(true, true), (368.0, 459.0));
    }

    #[test]
    fn pins_the_default_origin_to_the_work_area_top_right() {
        assert_eq!(
            layout::top_right_origin(0.0, 0.0, 1920.0, 1080.0, 111.0, 67.0),
            (1801.0, 8.0)
        );
    }

    #[test]
    fn keeps_the_top_right_anchor_when_resizing() {
        assert_eq!(
            layout::resize_keep_top_right(1801.0, 8.0, 111.0, 67.0, 368.0, 459.0),
            (1544.0, 8.0)
        );
    }

    #[test]
    fn clamps_an_offscreen_window_back_into_the_work_area() {
        assert_eq!(
            layout::clamp_to_work_area(1900.0, -20.0, 368.0, 459.0, 0.0, 0.0, 1920.0, 1080.0),
            (1552.0, 0.0)
        );
    }
}
