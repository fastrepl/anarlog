use serde::{Deserialize, Serialize};

use crate::Error;
use crate::window::live_caption::LiveCaptionPosition;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FloatingBarStatus {
    Recording,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FloatingBarOverlayLayout {
    pub controls_center_x: f64,
    pub expands_upward: bool,
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
pub struct FloatingDictationState {
    pub session_id: String,
    pub phase: String,
    pub microphone: String,
    pub text: String,
    pub partial: String,
    pub preview_enabled: bool,
    pub preview_unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FloatingBarState {
    #[serde(default)]
    pub dictation: Option<FloatingDictationState>,
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
    #[serde(default)]
    pub layout: Option<FloatingBarOverlayLayout>,
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
    pub const COMPACT_GAP: f64 = 0.0;
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

    #[cfg(any(not(target_os = "macos"), test))]
    pub fn dictation_container_size(expanded: bool) -> (f64, f64) {
        container_size(expanded, true)
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

    #[cfg(any(test, not(target_os = "macos")))]
    pub fn controls_center_y(height: f64, expands_upward: bool) -> f64 {
        if expands_upward {
            height - INSET - COMPACT_HEIGHT / 2.0
        } else {
            INSET + HOVER_HANDLE_RESERVED_HEIGHT + COMPACT_HEIGHT / 2.0
        }
    }

    #[cfg(any(test, not(target_os = "macos")))]
    pub fn frame_at_controls(
        anchor: (f64, f64),
        size: (f64, f64),
        work: (f64, f64, f64, f64),
        expands_upward: bool,
    ) -> (f64, f64, f64, f64) {
        let width = size.0.min(work.2);
        let min_height = size.1.min(container_size(false, true).1).min(work.3);
        let (y, height) = if expands_upward {
            let bottom = (anchor.1 + INSET + COMPACT_HEIGHT / 2.0)
                .clamp(work.1 + min_height, work.1 + work.3);
            let height = size.1.min(bottom - work.1);
            (bottom - height, height)
        } else {
            let y = (anchor.1 - controls_center_y(size.1, false))
                .clamp(work.1, work.1 + work.3 - min_height);
            (y, size.1.min(work.1 + work.3 - y))
        };
        let x = (anchor.0 - width / 2.0).clamp(work.0, work.0 + work.2 - width);
        (x, y, width, height)
    }

    pub fn bottom_center_origin(
        work_x: f64,
        work_y: f64,
        work_width: f64,
        work_height: f64,
        window_width: f64,
        window_height: f64,
    ) -> (f64, f64) {
        (
            work_x + (work_width - window_width) / 2.0,
            work_y + work_height - window_height - SCREEN_MARGIN,
        )
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
    pub extern "C" fn rust_on_floating_bar_dictation_action(payload: *const c_char) {
        if payload.is_null() {
            return;
        }
        let Ok(json) = (unsafe { CStr::from_ptr(payload) }).to_str() else {
            return;
        };
        if let (Some(app), Ok(event)) = (
            APP_HANDLE.get(),
            serde_json::from_str::<crate::events::FloatingBarDictationAction>(json),
        ) {
            let _ = event.emit(app);
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

// Type-check the webview implementation in macOS tests as well.
#[cfg(any(test, not(target_os = "macos")))]
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod cross_platform {
    use std::sync::{Mutex, OnceLock};

    use tauri::{
        LogicalPosition, LogicalSize, Manager, Position, Size, WebviewUrl, WebviewWindow,
        WebviewWindowBuilder, window::Color,
    };
    use tauri_specta::Event;

    use super::layout::{
        bottom_center_origin, container_size, controls_center_y, frame_at_controls, is_expanded,
    };
    use super::{FloatingBarOverlayLayout, FloatingBarState, WINDOW_LABEL};
    use crate::Error;
    use crate::ext::run_on_main_thread;

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
        run_on_main_thread(app, move || {
            let window = ensure_window(app)?;
            let mut state = current_state();
            let layout = apply_layout(&window, state.as_ref(), true)?;
            if let Some(state) = state.as_mut() {
                state.layout = Some(layout);
            }
            if let Some(state) = state {
                publish_state(state)?;
            }
            window.show()?;
            crate::window::exclude_from_capture(&window);
            Ok(())
        })?
    }

    pub fn hide() -> Result<(), Error> {
        if let Ok(app) = app() {
            run_on_main_thread(app, move || -> Result<(), Error> {
                if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                    window.hide()?;
                }
                Ok(())
            })??;
        }
        if let Ok(mut state) = LAST_STATE.lock() {
            *state = None;
        }
        Ok(())
    }

    pub fn update(mut state: FloatingBarState) -> Result<(), Error> {
        let app = app()?;
        run_on_main_thread(app, move || {
            if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                state.layout = Some(apply_layout(&window, Some(&state), false)?);
            }
            publish_state(state)
        })?
    }

    fn publish_state(state: FloatingBarState) -> Result<(), Error> {
        if let Ok(mut last) = LAST_STATE.lock() {
            *last = Some(state.clone());
        }
        let _ = crate::events::FloatingBarOverlayState { state }.emit(app()?);
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
        let builder = WebviewWindowBuilder::new(
            app,
            WINDOW_LABEL,
            WebviewUrl::App("app/floating-bar".into()),
        )
        .title("Anarlog")
        .inner_size(width, height)
        .visible(false)
        .focused(false)
        .decorations(false);
        #[cfg(any(not(target_os = "macos"), feature = "macos-private-api"))]
        let builder = builder.transparent(true);
        let window = builder
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
    ) -> Result<FloatingBarOverlayLayout, Error> {
        let is_expanded = state.is_some_and(is_expanded);
        let shows_expand = state.is_some_and(|value| value.live_caption_toggle_visible);
        let dictation = state.is_some_and(|state| state.dictation.is_some());
        window.set_focusable(!dictation)?;
        let (width, height) = if dictation {
            super::layout::dictation_container_size(is_expanded)
        } else {
            container_size(is_expanded, shows_expand)
        };
        let next_size = LogicalSize::new(width, height);
        let scale = window.scale_factor()?;
        let current_size = window.outer_size()?.to_logical::<f64>(scale);
        let current_position = window.outer_position()?.to_logical::<f64>(scale);
        let size = (next_size.width, next_size.height);
        let previous = current_state();
        let old_layout =
            previous
                .as_ref()
                .and_then(|state| state.layout)
                .unwrap_or(FloatingBarOverlayLayout {
                    controls_center_x: current_size.width / 2.0,
                    expands_upward: true,
                });
        if force_default_position {
            let (x, y) = default_origin(window, size.0, size.1)?;
            window.set_size(Size::Logical(LogicalSize::new(size.0, size.1)))?;
            window.set_position(Position::Logical(LogicalPosition::new(x, y)))?;
            return Ok(FloatingBarOverlayLayout {
                controls_center_x: size.0 / 2.0,
                expands_upward: true,
            });
        }
        let anchor = (
            current_position.x + old_layout.controls_center_x,
            current_position.y + controls_center_y(current_size.height, old_layout.expands_upward),
        );
        let monitor = window
            .current_monitor()?
            .or(window.app_handle().primary_monitor()?)
            .ok_or(Error::MonitorNotFound)?;
        let work = monitor.work_area();
        let origin = work.position.to_logical::<f64>(monitor.scale_factor());
        let work_size = work.size.to_logical::<f64>(monitor.scale_factor());
        let grows = is_expanded && !previous.as_ref().is_some_and(super::layout::is_expanded);
        let upwards = if grows {
            anchor.1 - origin.y > origin.y + work_size.height - anchor.1
        } else {
            old_layout.expands_upward
        };
        let frame = frame_at_controls(
            anchor,
            size,
            (origin.x, origin.y, work_size.width, work_size.height),
            upwards,
        );
        if (current_size.width - frame.2).abs() >= 0.5
            || (current_size.height - frame.3).abs() >= 0.5
        {
            window.set_size(Size::Logical(LogicalSize::new(frame.2, frame.3)))?;
        }
        if (current_position.x - frame.0).abs() >= 0.5
            || (current_position.y - frame.1).abs() >= 0.5
        {
            window.set_position(Position::Logical(LogicalPosition::new(frame.0, frame.1)))?;
        }
        Ok(FloatingBarOverlayLayout {
            controls_center_x: anchor.0 - frame.0,
            expands_upward: upwards,
        })
    }

    fn default_origin(
        window: &WebviewWindow<tauri::Wry>,
        width: f64,
        height: f64,
    ) -> Result<(f64, f64), Error> {
        let pointer_monitor = current_state()
            .is_some_and(|state| state.dictation.is_some())
            .then(|| {
                window
                    .app_handle()
                    .cursor_position()
                    .ok()
                    .and_then(|cursor| {
                        window
                            .app_handle()
                            .monitor_from_point(cursor.x, cursor.y)
                            .ok()
                            .flatten()
                    })
            })
            .flatten();
        let monitor = pointer_monitor
            .or_else(|| window.current_monitor().ok().flatten())
            .or_else(|| window.app_handle().primary_monitor().ok().flatten())
            .ok_or(Error::MonitorNotFound)?;
        let scale = monitor.scale_factor();
        let work_area = monitor.work_area();
        let origin = work_area.position.to_logical::<f64>(scale);
        let size = work_area.size.to_logical::<f64>(scale);
        Ok(bottom_center_origin(
            origin.x,
            origin.y,
            size.width,
            size.height,
            width,
            height,
        ))
    }
}

#[cfg(not(target_os = "macos"))]
use cross_platform as platform;

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
    fn controls_dragged_near_vertical_edges_keep_the_panel_visible() {
        for anchor_y in [-100.0, 10.0, 1070.0, 1200.0] {
            for upwards in [false, true] {
                let (x, y, width, height) = layout::frame_at_controls(
                    (960.0, anchor_y),
                    (368.0, 459.0),
                    (0.0, 0.0, 1920.0, 1080.0),
                    upwards,
                );
                assert!(x >= 0.0 && y >= 0.0);
                assert!(height >= layout::container_size(false, true).1);
                assert!(x + width <= 1920.0 && y + height <= 1080.0);
            }
        }
    }

    #[test]
    fn dictation_sizes_preserve_the_shared_panel_anchors() {
        assert_eq!(layout::dictation_container_size(false), (108.0, 67.0));
        assert_eq!(layout::dictation_container_size(true), (368.0, 459.0));
    }
    #[test]
    fn sizes_the_compact_and_expanded_windows() {
        assert_eq!(layout::container_size(false, false), (84.0, 67.0));
        assert_eq!(layout::container_size(false, true), (108.0, 67.0));
        assert_eq!(layout::container_size(true, true), (368.0, 459.0));
    }

    #[test]
    fn controls_keep_their_screen_position_through_expansion_and_collapse() {
        let work = (-1920.0, 40.0, 1920.0, 1040.0);
        for (anchor, upwards) in [
            ((-960.0, 1049.0), true),
            ((-960.0, 92.0), false),
            ((-64.0, 1049.0), true),
        ] {
            for size in [(111.0, 67.0), (368.0, 459.0), (111.0, 67.0)] {
                let frame = layout::frame_at_controls(anchor, size, work, upwards);
                assert_eq!(
                    frame.1 + layout::controls_center_y(frame.3, upwards),
                    anchor.1
                );
                assert!(frame.0 <= anchor.0 && frame.0 + frame.2 >= anchor.0);
                assert!(frame.0 >= work.0 && frame.0 + frame.2 <= work.0 + work.2);
                assert!(frame.1 >= work.1 && frame.1 + frame.3 <= work.1 + work.3);
            }
        }
    }

    #[test]
    fn short_displays_reduce_the_panel_instead_of_moving_the_controls() {
        let frame = layout::frame_at_controls(
            (150.0, 180.0),
            (368.0, 459.0),
            (0.0, 0.0, 300.0, 320.0),
            true,
        );
        assert_eq!(frame, (0.0, 0.0, 300.0, 203.0));
        assert_eq!(frame.1 + layout::controls_center_y(frame.3, true), 180.0);
    }

    #[test]
    fn collapse_after_drag_uses_the_new_control_position() {
        let work = (0.0, 0.0, 1920.0, 1080.0);
        let expanded = layout::frame_at_controls((1856.0, 1049.0), (368.0, 459.0), work, true);
        let controls_x = 1856.0 - expanded.0;
        let moved_anchor = (expanded.0 - 100.0 + controls_x, 1049.0);
        let collapsed = layout::frame_at_controls(moved_anchor, (144.0, 67.0), work, true);
        assert_eq!(collapsed.0 + collapsed.2 / 2.0, 1756.0);
    }

    #[test]
    fn default_origin_is_bottom_center_on_offset_displays() {
        assert_eq!(
            layout::bottom_center_origin(-1920.0, 40.0, 1920.0, 1040.0, 111.0, 67.0),
            (-1015.5, 1005.0)
        );
    }
}
