/// What runs once the app has quit through the drain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Restart {
    /// The app again, from the package it was launched from.
    App,
    /// A downloaded NSIS update, which replaces the install and then relaunches the app.
    Installer(std::path::PathBuf),
}

static RESTART: std::sync::Mutex<Option<Restart>> = std::sync::Mutex::new(None);
static LAUNCH: std::sync::OnceLock<(Option<std::path::PathBuf>, Vec<std::ffi::OsString>)> =
    std::sync::OnceLock::new();

pub fn request_restart(restart: Restart) {
    *RESTART.lock().unwrap_or_else(|error| error.into_inner()) = Some(restart);
}

/// What a restart relaunches and with which arguments, recorded at launch before an update
/// can replace the executable.
pub fn remember_launch(executable: Option<std::path::PathBuf>, arguments: Vec<std::ffi::OsString>) {
    let _ = LAUNCH.set((executable, arguments));
}

/// Matches `tauri-plugin-updater`'s `escape_nsis_current_exe_arg` so NSIS's `GetOptions` parser
/// keeps values containing `/`, spaces, or tabs intact after `/ARGS`.
#[cfg(any(windows, test))]
fn escape_nsis_arg(arg: &std::ffi::OsStr) -> String {
    let arg = arg.to_string_lossy();
    let quote = arg.is_empty() || arg.chars().any(|c| c == ' ' || c == '\t' || c == '/');
    let mut escaped = String::with_capacity(arg.len() + 2);
    if quote {
        escaped.push('"');
    }
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        if ch == '\\' {
            backslashes += 1;
        } else {
            if ch == '"' {
                for _ in 0..=backslashes {
                    escaped.push('\\');
                }
            }
            backslashes = 0;
        }
        escaped.push(ch);
    }
    if quote {
        for _ in 0..backslashes {
            escaped.push('\\');
        }
        escaped.push('"');
    }
    escaped
}

/// Starts the requested restart, once. macOS runs it just before quitting, since quitting there
/// ends the process; Linux and Windows run it after the run loop has returned and released the
/// single-instance claim.
pub fn run_requested_restart() -> std::io::Result<()> {
    let restart = RESTART
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    let Some(restart) = restart else {
        return Ok(());
    };
    let (executable, arguments) = LAUNCH.get().cloned().unwrap_or_default();
    let mut command = match restart {
        Restart::App => {
            let mut command = std::process::Command::new(match executable {
                Some(executable) => executable,
                None => std::env::current_exe()?,
            });
            command.args(arguments);
            command
        }
        Restart::Installer(installer) => {
            let mut command = std::process::Command::new(installer);
            command.args(crate::platform::updater::NSIS_UPDATE_ARGS);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                for arg in &arguments {
                    command.raw_arg(escape_nsis_arg(arg));
                }
            }
            #[cfg(not(windows))]
            command.args(arguments);
            command
        }
    };
    command.spawn().map(drop)
}

use desktop_runtime::{Result, ServiceError, SessionId};
use gpui::{
    App, Bounds, Pixels, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowKind,
    WindowOptions, point, px, size,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WindowIdentity {
    Main,
    Note(SessionId),
    Composer,
    FloatingBar,
    LiveCaption,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Frame {
    fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.width > 0.0
            && self.height > 0.0
    }

    fn overlap(self, other: Self) -> (f32, f32) {
        (
            ((self.x + self.width).min(other.x + other.width) - self.x.max(other.x)).max(0.0),
            ((self.y + self.height).min(other.y + other.height) - self.y.max(other.y)).max(0.0),
        )
    }

    fn bounds(self) -> Bounds<Pixels> {
        Bounds::new(
            point(px(self.x), px(self.y)),
            size(px(self.width), px(self.height)),
        )
    }
}

/// Frames and work areas use physical pixels; scales describe the saved and current display.
pub fn restore(
    saved: Frame,
    old_scale: f32,
    new_scale: f32,
    work_areas: &[Frame],
) -> Result<Frame> {
    if !saved.valid()
        || !old_scale.is_finite()
        || !new_scale.is_finite()
        || old_scale <= 0.0
        || new_scale <= 0.0
        || work_areas.is_empty()
        || work_areas.iter().any(|area| !area.valid())
    {
        return Err(ServiceError::Failed(
            "Invalid saved window geometry or display configuration".into(),
        ));
    }
    let mut frame = Frame {
        width: saved.width / old_scale * new_scale,
        height: saved.height / old_scale * new_scale,
        ..saved
    };
    let titlebar = Frame {
        height: frame.height.min(32.0 * new_scale),
        ..frame
    };
    if work_areas.iter().any(|area| {
        let (width, height) = titlebar.overlap(*area);
        width >= titlebar.width.min(128.0 * new_scale)
            && height >= titlebar.height.min(16.0 * new_scale)
    }) {
        return Ok(frame);
    }
    let area = work_areas
        .iter()
        .max_by(|a, b| {
            let (aw, ah) = frame.overlap(**a);
            let (bw, bh) = frame.overlap(**b);
            (aw * ah).total_cmp(&(bw * bh))
        })
        .filter(|area| {
            let (w, h) = frame.overlap(**area);
            w * h > 0.0
        })
        .unwrap_or(&work_areas[0]);
    frame.x = area.x + (area.width - frame.width).max(0.0) / 2.0;
    frame.y = area.y + (area.height - frame.height).max(0.0) / 2.0;
    Ok(frame)
}

impl WindowIdentity {
    pub fn label(&self) -> String {
        match self {
            Self::Main => "main".into(),
            Self::Note(id) => format!("note-{}", id.0),
            Self::Composer => "composer".into(),
            Self::FloatingBar => "floating-bar".into(),
            Self::LiveCaption => "live-caption".into(),
        }
    }

    pub fn options(&self, restored_logical: Option<Frame>, cx: &App) -> Result<WindowOptions> {
        let (width, height, minimum, kind) = match self {
            Self::Main => (910.0, 600.0, (500.0, 500.0), WindowKind::Normal),
            Self::Note(_) => (720.0, 820.0, (420.0, 500.0), WindowKind::Normal),
            Self::Composer => (640.0, 180.0, (400.0, 120.0), WindowKind::Floating),
            Self::FloatingBar => (320.0, 64.0, (280.0, 64.0), WindowKind::PopUp),
            Self::LiveCaption => (640.0, 180.0, (320.0, 100.0), WindowKind::Floating),
        };
        let bounds = match restored_logical {
            Some(frame) if frame.valid() => frame.bounds(),
            Some(_) => return Err(ServiceError::Failed("Invalid window bounds".into())),
            None => Bounds::centered(None, size(px(width), px(height)), cx),
        };
        Ok(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(minimum.0), px(minimum.1))),
            window_decorations: matches!(self, Self::Main).then_some(WindowDecorations::Client),
            kind,
            focus: !matches!(self, Self::FloatingBar | Self::LiveCaption),
            is_resizable: !matches!(self, Self::FloatingBar),
            is_minimizable: matches!(self, Self::Main | Self::Note(_)),
            titlebar: if matches!(self, Self::Main) && !cfg!(target_os = "macos") {
                None
            } else {
                Some(TitlebarOptions {
                    appears_transparent: cfg!(target_os = "macos"),
                    // Matches the shipping window's traffic lights (Tauri offset 12, 25).
                    traffic_light_position: matches!(self, Self::Main | Self::Note(_))
                        .then(|| point(px(12.), px(16.))),
                    ..Default::default()
                })
            },
            app_id: crate::platform::package::window_app_id(),
            ..Default::default()
        })
    }
}

/// Borderless `NSWindowStyleMaskNonactivatingPanel` (`1 << 7`), omitting `NSWindowStyleMaskTitled`
/// and `FullSizeContentView` so macOS renders no titled chrome or liquid-glass wrapper.
#[cfg(target_os = "macos")]
const FLOATING_PANEL_STYLE_MASK: usize = 1 << 7;
/// `CanJoinAllSpaces | Stationary | FullScreenAuxiliary` (`FloatingBarManager.swift`).
#[cfg(target_os = "macos")]
const FLOATING_PANEL_COLLECTION_BEHAVIOR: usize = (1 << 0) | (1 << 4) | (1 << 8);

/// Removes native titled panel chrome, backdrop, and shadow on macOS (`FloatingBarManager.swift`)
/// and excludes the window from Windows screen capture (`WDA_EXCLUDEFROMCAPTURE`).
///
/// Apple deprecated `NSWindowSharingNone` and ScreenCaptureKit does not honor an AppKit window
/// exclusion flag on modern macOS, so macOS screen shares can still capture floating panels.
pub fn configure_floating_panel(window: &Window) {
    #[cfg(target_os = "macos")]
    macos::configure_floating_panel(window);
    #[cfg(target_os = "windows")]
    win32::exclude_from_capture(window);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = window;
}

/// Hides only the main window like shipping's `AppWindow::Main.hide` (`orderOut:`), leaving
/// note windows open; the next tray or Dock action brings it back.
pub fn hide_main(window: &Window, cx: &App) -> Result<()> {
    if window.is_fullscreen() {
        window.toggle_fullscreen();
    }
    #[cfg(target_os = "macos")]
    {
        // AppKit reports the resulting focus change synchronously, so order out after GPUI
        // has released the window.
        let native = macos::ns_window(window)?;
        cx.foreground_executor()
            .spawn(async move { macos::order_out(&native) })
            .detach();
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        let _ = cx;
        win32::show(window, windows::Win32::UI::WindowsAndMessaging::SW_HIDE)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = cx;
        linux::set_visible(window, false)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = cx;
        Err(ServiceError::Unsupported(
            "This platform cannot hide a single window".into(),
        ))
    }
}

/// Shows a main window `hide_main` hid, before GPUI activates it: GPUI's Windows activation
/// restores minimized windows but does not show hidden ones, and its X11 activation only asks
/// the window manager for focus.
pub fn show_main(window: &Window) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        win32::show(window, windows::Win32::UI::WindowsAndMessaging::SW_SHOW)
    }
    #[cfg(target_os = "linux")]
    {
        linux::set_visible(window, true)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = window;
        Ok(())
    }
}

/// GPUI's pinned Windows backend only maximizes; the titlebar must also restore.
pub fn toggle_maximize(window: &Window) {
    #[cfg(target_os = "windows")]
    if let Err(error) = win32::toggle_maximize(window) {
        tracing::warn!(%error, "Could not toggle window maximization");
    }
    #[cfg(not(target_os = "windows"))]
    window.zoom_window();
}

#[cfg(target_os = "linux")]
mod linux {
    use desktop_runtime::{Result, ServiceError};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};

    /// What GPUI still has to do after the native visibility change.
    #[derive(Debug, PartialEq)]
    enum Followup {
        None,
        /// Wayland cannot hide a toplevel. Minimizing is the closest match to shipping's hide,
        /// and GPUI's activation asks the compositor to bring the window back.
        Minimize,
    }

    pub(super) fn set_visible(window: &gpui::Window, visible: bool) -> Result<()> {
        let handle = HasWindowHandle::window_handle(window)
            .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        if set_handle_visible(handle.as_raw(), visible)? == Followup::Minimize {
            window.minimize_window();
        }
        Ok(())
    }

    fn set_handle_visible(handle: RawWindowHandle, visible: bool) -> Result<Followup> {
        let handle = match handle {
            RawWindowHandle::Xcb(handle) => handle,
            RawWindowHandle::Wayland(_) if !visible => return Ok(Followup::Minimize),
            // GPUI's activation shows these backends' windows.
            _ if visible => return Ok(Followup::None),
            _ => {
                return Err(ServiceError::Unsupported(
                    "Hiding a window to the tray is not supported by this display backend".into(),
                ));
            }
        };
        let failed = |error: &dyn std::fmt::Display| ServiceError::Failed(error.to_string().into());
        let (connection, _) = x11rb::connect(None).map_err(|error| failed(&error))?;
        // GPUI's X11 activate() only asks the WM for focus; it does not map a hidden window.
        let request = if visible {
            connection.map_window(handle.window.get())
        } else {
            connection.unmap_window(handle.window.get())
        }
        .map_err(|error| failed(&error))?;
        request.check().map_err(|error| failed(&error))?;
        connection.flush().map_err(|error| failed(&error))?;
        Ok(Followup::None)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn closing_a_wayland_window_minimizes_it_instead_of_failing() {
            let handle = RawWindowHandle::Wayland(raw_window_handle::WaylandWindowHandle::new(
                std::ptr::NonNull::dangling(),
            ));
            assert_eq!(
                set_handle_visible(handle, false).unwrap(),
                Followup::Minimize
            );
            assert_eq!(set_handle_visible(handle, true).unwrap(), Followup::None);
        }

        #[test]
        #[ignore = "Requires a live X11 display; https://linear.app/fastrepl-inc/issue/ANLG-320"]
        fn close_to_tray_unmaps_and_reopens_the_same_x11_window() {
            use x11rb::protocol::xproto::{CreateWindowAux, MapState, WindowClass};

            let (connection, screen) = x11rb::connect(None).unwrap();
            let root = connection.setup().roots[screen].root;
            let window = connection.generate_id().unwrap();
            connection
                .create_window(
                    x11rb::COPY_DEPTH_FROM_PARENT,
                    window,
                    root,
                    0,
                    0,
                    32,
                    32,
                    0,
                    WindowClass::INPUT_OUTPUT,
                    0,
                    &CreateWindowAux::new().override_redirect(1),
                )
                .unwrap()
                .check()
                .unwrap();
            let handle = RawWindowHandle::Xcb(raw_window_handle::XcbWindowHandle::new(
                std::num::NonZeroU32::new(window).unwrap(),
            ));
            let state = || {
                connection
                    .get_window_attributes(window)
                    .unwrap()
                    .reply()
                    .unwrap()
                    .map_state
            };
            set_handle_visible(handle, true).unwrap();
            assert_eq!(state(), MapState::VIEWABLE);
            set_handle_visible(handle, false).unwrap();
            assert_eq!(state(), MapState::UNMAPPED);
            set_handle_visible(handle, true).unwrap();
            assert_eq!(state(), MapState::VIEWABLE);
            connection.destroy_window(window).unwrap().check().unwrap();
        }
    }
}

/// The standard Close Window item (`performClose:`): runs the window's own close handler.
pub fn perform_close(window: &Window, cx: &App) {
    #[cfg(target_os = "macos")]
    if let Ok(native) = macos::ns_window(window) {
        cx.foreground_executor()
            .spawn(async move { macos::perform_close(&native) })
            .detach();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, cx);
}

/// Changes a window's minimum content size after it opened, as shipping's zoom does
/// (`setMinSize`); GPUI only sets it when the window opens.
pub fn set_min_size(window: &Window, minimum: gpui::Size<Pixels>) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        appkit::set_content_min_size(&*macos::ns_window(window)?, minimum);
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, minimum);
        Err(ServiceError::Unsupported(
            "This platform cannot change a window's minimum size".into(),
        ))
    }
}

#[cfg(target_os = "windows")]
mod win32 {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            IsZoomed, SHOW_WINDOW_CMD, SW_MAXIMIZE, SW_RESTORE, SetWindowDisplayAffinity,
            ShowWindowAsync, WDA_EXCLUDEFROMCAPTURE,
        },
    };

    use super::{Result, ServiceError, Window};

    fn hwnd(window: &Window) -> Result<HWND> {
        let unavailable = || ServiceError::Unsupported("The native window is unavailable".into());
        let handle = HasWindowHandle::window_handle(window).map_err(|_| unavailable())?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err(unavailable());
        };
        Ok(HWND(handle.hwnd.get() as *mut _))
    }

    pub fn exclude_from_capture(window: &Window) {
        let Ok(native) = hwnd(window) else {
            return;
        };
        // SAFETY: GPUI owns this open HWND on its UI thread.
        if let Err(error) = unsafe { SetWindowDisplayAffinity(native, WDA_EXCLUDEFROMCAPTURE) } {
            tracing::warn!(%error, "Could not exclude floating panel from screen capture");
        }
    }

    pub fn show(window: &Window, command: SHOW_WINDOW_CMD) -> Result<()> {
        let native = hwnd(window)?;
        // SAFETY: GPUI owns this open HWND. Post the visibility change to avoid reentering
        // its window callbacks while the application view is borrowed.
        if !unsafe { ShowWindowAsync(native, command) }.as_bool() {
            return Err(ServiceError::Failed(
                "Could not change native window visibility".into(),
            ));
        }
        Ok(())
    }

    pub fn toggle_maximize(window: &Window) -> Result<()> {
        // SAFETY: GPUI owns the open HWND; IsZoomed only reads its maximized state.
        let maximized = unsafe { IsZoomed(hwnd(window)?) }.as_bool();
        show(window, if maximized { SW_RESTORE } else { SW_MAXIMIZE })
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use desktop_runtime::{Result, ServiceError};
    use objc2::{
        class, msg_send,
        rc::Retained,
        runtime::{AnyObject, Bool},
        sel,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use super::{FLOATING_PANEL_COLLECTION_BEHAVIOR, FLOATING_PANEL_STYLE_MASK};

    pub fn ns_window(window: &gpui::Window) -> Result<Retained<AnyObject>> {
        let unavailable = || ServiceError::Unsupported("The native window is unavailable".into());
        let handle = HasWindowHandle::window_handle(window).map_err(|_| unavailable())?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err(unavailable());
        };
        let view = handle.ns_view.as_ptr().cast::<AnyObject>();
        // SAFETY: the view belongs to this open window; retaining keeps its NSWindow alive.
        unsafe {
            let native: *mut AnyObject = msg_send![view, window];
            Retained::retain(native).ok_or_else(unavailable)
        }
    }

    pub fn configure_floating_panel(window: &gpui::Window) {
        let Ok(panel) = ns_window(window) else {
            return;
        };
        configure_native_floating_panel(&panel, true);
    }

    pub(super) fn configure_native_floating_panel(panel: &AnyObject, order_front: bool) {
        // SAFETY: AppKit runs panel updates on the main thread on a live NSWindow/NSPanel.
        // Changing `styleMask` strips `NSWindowStyleMaskTitled` and `FullSizeContentView` (which
        // cause macOS 26 Tahoe to paint a rounded liquid-glass wrapper around transparent
        // windows) and can reset window level and movability, so restore both.
        unsafe {
            let level: isize = msg_send![panel, level];
            let movable: Bool = msg_send![panel, isMovable];
            let _: () = msg_send![panel, setStyleMask: FLOATING_PANEL_STYLE_MASK];
            let _: () = msg_send![panel, setLevel: level];
            let _: () = msg_send![panel, setMovable: movable];
            let _: () = msg_send![panel, setOpaque: Bool::NO];
            let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
            let _: () = msg_send![panel, setBackgroundColor: clear];
            let _: () = msg_send![panel, setHasShadow: Bool::NO];
            let _: () = msg_send![panel, invalidateShadow];
            let _: () = msg_send![panel, setHidesOnDeactivate: Bool::NO];
            let _: () = msg_send![panel, setCollectionBehavior: FLOATING_PANEL_COLLECTION_BEHAVIOR];
            let is_panel: Bool = msg_send![panel, respondsToSelector: sel!(setFloatingPanel:)];
            if is_panel.as_bool() {
                let _: () = msg_send![panel, setFloatingPanel: Bool::YES];
            }
            if order_front {
                let _: () = msg_send![panel, orderFrontRegardless];
            }
        }
    }

    pub fn order_out(window: &AnyObject) {
        // SAFETY: any NSWindow accepts orderOut: with a nil sender.
        unsafe {
            let _: () = msg_send![window, orderOut: std::ptr::null_mut::<AnyObject>()];
        }
    }

    pub fn perform_close(window: &AnyObject) {
        // SAFETY: any NSWindow accepts performClose: with a nil sender.
        unsafe {
            let _: () = msg_send![window, performClose: std::ptr::null_mut::<AnyObject>()];
        }
    }
}

/// The main window's geometry for the browser hand-off (shipping's `windowSaveFrame`,
/// `windowSetFrameAnimated` and `windowRestoreFrameAnimated`), in the platform's screen
/// coordinates.
pub trait FrameWindow {
    fn frame(&self) -> Result<Frame>;
    /// The screen's usable area, without the menu bar and Dock.
    fn visible_frame(&self) -> Result<Frame>;
    fn is_maximized(&self) -> bool;
    fn set_maximized(&self, maximized: bool);
    fn set_frame_animated(&self, frame: Frame);
    fn set_always_on_top(&self, on_top: bool);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavedFrame {
    pub frame: Frame,
    pub maximized: bool,
}

const ANCHOR_MARGIN: f32 = 8.0;

/// Saves the frame to restore; a maximized window is un-maximized first, as shipping does.
pub fn save_frame(window: &dyn FrameWindow) -> Result<SavedFrame> {
    let maximized = window.is_maximized();
    if maximized {
        window.set_maximized(false);
    }
    let frame = window.frame().and_then(|frame| {
        frame
            .valid()
            .then_some(frame)
            .ok_or_else(|| ServiceError::Failed("Invalid window frame".into()))
    });
    if frame.is_err() && maximized {
        window.set_maximized(true);
    }
    Ok(SavedFrame {
        frame: frame?,
        maximized,
    })
}

/// Animates the window to a `width`×`height` card in the screen's top-right corner, above
/// other windows.
pub fn show_top_right(window: &dyn FrameWindow, width: f32, height: f32) -> Result<()> {
    if window.is_maximized() {
        window.set_maximized(false);
    }
    window.set_always_on_top(true);
    let screen = window.visible_frame()?;
    window.set_frame_animated(Frame {
        x: screen.x + screen.width - width - ANCHOR_MARGIN,
        // AppKit measures y from the bottom of the screen.
        y: if cfg!(target_os = "macos") {
            screen.y + screen.height - height - ANCHOR_MARGIN
        } else {
            screen.y + ANCHOR_MARGIN
        },
        width,
        height,
    });
    Ok(())
}

/// Animates back to the saved frame. Without one only the window level is reset, as in
/// shipping.
pub fn restore_frame(window: &dyn FrameWindow, saved: Option<SavedFrame>) {
    if let Some(saved) = saved {
        if window.is_maximized() {
            window.set_maximized(false);
        }
        window.set_frame_animated(saved.frame);
        if saved.maximized {
            window.set_maximized(true);
        }
    }
    window.set_always_on_top(false);
}

/// The window's native frame, retained so callers can drive it outside GPUI's window update:
/// AppKit reports resizes synchronously, and GPUI drops them while the app is borrowed.
/// Other platforms have no adapter, so the hand-off keeps the window as it is.
pub fn frame_window(window: &Window) -> Option<Box<dyn FrameWindow>> {
    #[cfg(target_os = "macos")]
    {
        appkit::AppKitWindow::of(window).map(|window| Box::new(window) as Box<dyn FrameWindow>)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        None
    }
}

#[cfg(target_os = "macos")]
mod appkit {
    use objc2::{
        encode::{Encode, Encoding},
        msg_send,
        rc::Retained,
        runtime::{AnyObject, Bool},
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use super::{FrameWindow, Result, ServiceError};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    // SAFETY: these match CoreGraphics' 64-bit layouts and encodings.
    unsafe impl Encode for CGPoint {
        const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl Encode for CGSize {
        const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl Encode for CGRect {
        const ENCODING: Encoding =
            Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
    }

    impl From<CGRect> for super::Frame {
        fn from(rect: CGRect) -> Self {
            Self {
                x: rect.origin.x as f32,
                y: rect.origin.y as f32,
                width: rect.size.width as f32,
                height: rect.size.height as f32,
            }
        }
    }

    const FULL_SCREEN: usize = 1 << 14;
    const NORMAL_LEVEL: isize = 0;
    const FLOATING_LEVEL: isize = 3;

    pub fn set_content_min_size(window: &AnyObject, minimum: gpui::Size<gpui::Pixels>) {
        let size = CGSize {
            width: f32::from(minimum.width).into(),
            height: f32::from(minimum.height).into(),
        };
        // SAFETY: any NSWindow accepts setContentMinSize: on the main thread, where GPUI runs.
        unsafe {
            let _: () = msg_send![window, setContentMinSize: size];
        }
    }

    pub struct AppKitWindow(Retained<AnyObject>);

    impl AppKitWindow {
        pub fn of(window: &gpui::Window) -> Option<Self> {
            let RawWindowHandle::AppKit(handle) =
                HasWindowHandle::window_handle(window).ok()?.as_raw()
            else {
                return None;
            };
            let view = handle.ns_view.as_ptr().cast::<AnyObject>();
            // SAFETY: the view belongs to this open window; retaining keeps its NSWindow alive.
            unsafe {
                let ns_window: *mut AnyObject = msg_send![view, window];
                Retained::retain(ns_window).map(Self)
            }
        }
    }

    impl FrameWindow for AppKitWindow {
        fn frame(&self) -> Result<super::Frame> {
            // SAFETY: plain NSWindow getters on a retained window, on the main thread.
            unsafe {
                let style: usize = msg_send![&*self.0, styleMask];
                if style & FULL_SCREEN != 0 {
                    return Err(ServiceError::Unsupported(
                        "A full-screen window keeps its frame".into(),
                    ));
                }
                let frame: CGRect = msg_send![&*self.0, frame];
                Ok(frame.into())
            }
        }

        fn visible_frame(&self) -> Result<super::Frame> {
            // SAFETY: NSWindow/NSScreen getters on the main thread; nil screens are checked.
            unsafe {
                let mut screen: *mut AnyObject = msg_send![&*self.0, screen];
                if screen.is_null() {
                    screen = msg_send![objc2::class!(NSScreen), mainScreen];
                }
                if screen.is_null() {
                    return Err(ServiceError::Unsupported("No screen".into()));
                }
                let frame: CGRect = msg_send![screen, visibleFrame];
                Ok(frame.into())
            }
        }

        fn is_maximized(&self) -> bool {
            // SAFETY: NSWindow getter on the main thread.
            let zoomed: Bool = unsafe { msg_send![&*self.0, isZoomed] };
            zoomed.as_bool()
        }

        fn set_maximized(&self, maximized: bool) {
            if self.is_maximized() != maximized {
                // SAFETY: `zoom:` toggles the zoomed state and accepts a nil sender.
                unsafe {
                    let _: () = msg_send![&*self.0, zoom: std::ptr::null::<AnyObject>()];
                }
            }
        }

        fn set_frame_animated(&self, frame: super::Frame) {
            let rect = CGRect {
                origin: CGPoint {
                    x: f64::from(frame.x),
                    y: f64::from(frame.y),
                },
                size: CGSize {
                    width: f64::from(frame.width),
                    height: f64::from(frame.height),
                },
            };
            // SAFETY: NSWindow setter on the main thread. Unlike user resizing, this frame
            // change is not held to the window's minimum size, so the card can be narrower.
            unsafe {
                let _: () =
                    msg_send![&*self.0, setFrame: rect, display: Bool::YES, animate: Bool::YES];
            }
        }

        fn set_always_on_top(&self, on_top: bool) {
            let level = if on_top { FLOATING_LEVEL } else { NORMAL_LEVEL };
            // SAFETY: NSWindow setter on the main thread.
            unsafe {
                let _: () = msg_send![&*self.0, setLevel: level];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_respects_titlebar_scale_and_offscreen_recovery() {
        let screen = Frame {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1050.0,
        };
        let saved = Frame {
            x: 4000.0,
            y: 300.0,
            width: 1800.0,
            height: 1200.0,
        };
        assert_eq!(
            restore(saved, 2.0, 1.0, &[screen]).unwrap(),
            Frame {
                x: 510.0,
                y: 225.0,
                width: 900.0,
                height: 600.0
            }
        );
        let visible = Frame {
            x: 1792.0,
            y: 100.0,
            width: 900.0,
            height: 600.0,
        };
        assert_eq!(restore(visible, 1.0, 1.0, &[screen]).unwrap(), visible);
        assert_ne!(
            restore(
                Frame {
                    x: 1800.0,
                    ..visible
                },
                1.0,
                1.0,
                &[screen]
            )
            .unwrap()
            .x,
            1800.0
        );
        assert!(restore(saved, 0.0, 1.0, &[screen]).is_err());
    }

    #[test]
    fn nsis_relaunch_args_quote_forward_slashes_spaces_and_quotes() {
        use std::ffi::OsStr;

        assert_eq!(escape_nsis_arg(OsStr::new("--profile")), "--profile");
        assert_eq!(
            escape_nsis_arg(OsStr::new("C:/qa/profile")),
            "\"C:/qa/profile\""
        );
        assert_eq!(
            escape_nsis_arg(OsStr::new(r"C:\qa dir\")),
            r#""C:\qa dir\\""#
        );
        assert_eq!(escape_nsis_arg(OsStr::new(r#"a / "b""#)), r#""a / \"b\"""#);
        assert_eq!(escape_nsis_arg(OsStr::new(r#"a"b"#)), r#"a\"b"#);
        assert_eq!(escape_nsis_arg(OsStr::new("")), r#""""#);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn floating_panel_configuration_strips_titled_chrome_and_orders_front() {
        use std::{ffi::c_void, sync::OnceLock};

        use objc2::{
            class, msg_send,
            rc::Retained,
            runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel},
            sel,
        };

        #[derive(Clone, Copy)]
        struct OrderFrontSnapshot {
            style_mask: usize,
            level: isize,
            movable: bool,
            opaque: bool,
            background_alpha: f64,
            has_shadow: bool,
            shadow_invalidated: bool,
            hides_on_deactivate: bool,
            collection_behavior: usize,
            floating_panel: bool,
        }

        struct PanelProbe {
            style_mask: usize,
            level: isize,
            movable: bool,
            opaque: bool,
            background_alpha: f64,
            has_shadow: bool,
            shadow_invalidated: bool,
            hides_on_deactivate: bool,
            collection_behavior: usize,
            floating_panel: bool,
            at_order_front: Option<OrderFrontSnapshot>,
        }

        unsafe fn state(obj: *mut AnyObject) -> &'static mut PanelProbe {
            unsafe {
                let obj = &*obj;
                let ptr = *obj
                    .class()
                    .instance_variable(c"probe")
                    .expect("probe ivar")
                    .load_ptr::<*mut c_void>(obj);
                &mut *ptr.cast::<PanelProbe>()
            }
        }

        extern "C" fn level(obj: *mut AnyObject, _: Sel) -> isize {
            unsafe { state(obj).level }
        }
        extern "C" fn is_movable(obj: *mut AnyObject, _: Sel) -> Bool {
            Bool::new(unsafe { state(obj).movable })
        }
        extern "C" fn set_style_mask(obj: *mut AnyObject, _: Sel, mask: usize) {
            let probe = unsafe { state(obj) };
            probe.style_mask = mask;
            // Mirror AppKit resetting level and movability when styleMask changes.
            probe.level = 0;
            probe.movable = false;
        }
        extern "C" fn set_level(obj: *mut AnyObject, _: Sel, level: isize) {
            unsafe { state(obj).level = level };
        }
        extern "C" fn set_movable(obj: *mut AnyObject, _: Sel, movable: Bool) {
            unsafe { state(obj).movable = movable.as_bool() };
        }
        extern "C" fn set_opaque(obj: *mut AnyObject, _: Sel, opaque: Bool) {
            unsafe { state(obj).opaque = opaque.as_bool() };
        }
        extern "C" fn set_background_color(obj: *mut AnyObject, _: Sel, color: *mut AnyObject) {
            let alpha: f64 = unsafe { msg_send![color, alphaComponent] };
            unsafe { state(obj).background_alpha = alpha };
        }
        extern "C" fn set_has_shadow(obj: *mut AnyObject, _: Sel, shadow: Bool) {
            unsafe { state(obj).has_shadow = shadow.as_bool() };
        }
        extern "C" fn invalidate_shadow(obj: *mut AnyObject, _: Sel) {
            unsafe { state(obj).shadow_invalidated = true };
        }
        extern "C" fn set_hides_on_deactivate(obj: *mut AnyObject, _: Sel, hides: Bool) {
            unsafe { state(obj).hides_on_deactivate = hides.as_bool() };
        }
        extern "C" fn set_collection_behavior(obj: *mut AnyObject, _: Sel, behavior: usize) {
            unsafe { state(obj).collection_behavior = behavior };
        }
        extern "C" fn set_floating_panel(obj: *mut AnyObject, _: Sel, floating: Bool) {
            unsafe { state(obj).floating_panel = floating.as_bool() };
        }
        extern "C" fn order_front_regardless(obj: *mut AnyObject, _: Sel) {
            let probe = unsafe { state(obj) };
            probe.at_order_front = Some(OrderFrontSnapshot {
                style_mask: probe.style_mask,
                level: probe.level,
                movable: probe.movable,
                opaque: probe.opaque,
                background_alpha: probe.background_alpha,
                has_shadow: probe.has_shadow,
                shadow_invalidated: probe.shadow_invalidated,
                hides_on_deactivate: probe.hides_on_deactivate,
                collection_behavior: probe.collection_behavior,
                floating_panel: probe.floating_panel,
            });
        }

        static PANEL_CLASS: OnceLock<usize> = OnceLock::new();
        let class = *PANEL_CLASS.get_or_init(|| {
            let mut decl = ClassBuilder::new(c"AnarlogFloatingPanelProbe", class!(NSObject))
                .expect("unique Objective-C test class");
            decl.add_ivar::<*mut c_void>(c"probe");
            unsafe {
                decl.add_method(
                    sel!(level),
                    level as extern "C" fn(*mut AnyObject, Sel) -> isize,
                );
                decl.add_method(
                    sel!(isMovable),
                    is_movable as extern "C" fn(*mut AnyObject, Sel) -> Bool,
                );
                decl.add_method(
                    sel!(setStyleMask:),
                    set_style_mask as extern "C" fn(*mut AnyObject, Sel, usize),
                );
                decl.add_method(
                    sel!(setLevel:),
                    set_level as extern "C" fn(*mut AnyObject, Sel, isize),
                );
                decl.add_method(
                    sel!(setMovable:),
                    set_movable as extern "C" fn(*mut AnyObject, Sel, Bool),
                );
                decl.add_method(
                    sel!(setOpaque:),
                    set_opaque as extern "C" fn(*mut AnyObject, Sel, Bool),
                );
                decl.add_method(
                    sel!(setBackgroundColor:),
                    set_background_color as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
                );
                decl.add_method(
                    sel!(setHasShadow:),
                    set_has_shadow as extern "C" fn(*mut AnyObject, Sel, Bool),
                );
                decl.add_method(
                    sel!(invalidateShadow),
                    invalidate_shadow as extern "C" fn(*mut AnyObject, Sel),
                );
                decl.add_method(
                    sel!(setHidesOnDeactivate:),
                    set_hides_on_deactivate as extern "C" fn(*mut AnyObject, Sel, Bool),
                );
                decl.add_method(
                    sel!(setCollectionBehavior:),
                    set_collection_behavior as extern "C" fn(*mut AnyObject, Sel, usize),
                );
                decl.add_method(
                    sel!(setFloatingPanel:),
                    set_floating_panel as extern "C" fn(*mut AnyObject, Sel, Bool),
                );
                decl.add_method(
                    sel!(orderFrontRegardless),
                    order_front_regardless as extern "C" fn(*mut AnyObject, Sel),
                );
            }
            decl.register() as *const AnyClass as usize
        });

        let mut probe = PanelProbe {
            style_mask: (1 << 0) | (1 << 15) | (1 << 7),
            level: 101,
            movable: true,
            opaque: true,
            background_alpha: 1.0,
            has_shadow: true,
            shadow_invalidated: false,
            hides_on_deactivate: true,
            collection_behavior: 0,
            floating_panel: false,
            at_order_front: None,
        };
        let panel: Retained<AnyObject> = unsafe {
            let class = &*(class as *const AnyClass);
            let obj: Retained<AnyObject> = msg_send![class, new];
            obj.class()
                .instance_variable(c"probe")
                .unwrap()
                .load_ptr::<*mut c_void>(&obj)
                .write((&raw mut probe).cast());
            obj
        };
        macos::configure_native_floating_panel(&panel, true);
        drop(panel);

        let at_order_front = probe.at_order_front.expect("orderFrontRegardless called");
        assert_eq!(at_order_front.style_mask, FLOATING_PANEL_STYLE_MASK);
        assert_eq!(at_order_front.level, 101);
        assert!(at_order_front.movable);
        assert!(!at_order_front.opaque);
        assert_eq!(at_order_front.background_alpha, 0.0);
        assert!(!at_order_front.has_shadow);
        assert!(at_order_front.shadow_invalidated);
        assert!(!at_order_front.hides_on_deactivate);
        assert_eq!(
            at_order_front.collection_behavior,
            FLOATING_PANEL_COLLECTION_BEHAVIOR
        );
        assert!(at_order_front.floating_panel);
    }
}
