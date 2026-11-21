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
        Restart::App => std::process::Command::new(match executable {
            Some(executable) => executable,
            None => std::env::current_exe()?,
        }),
        Restart::Installer(installer) => {
            let mut command = std::process::Command::new(installer);
            command.args(crate::platform::updater::NSIS_UPDATE_ARGS);
            command
        }
    };
    command.args(arguments).spawn().map(drop)
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
            IsZoomed, SHOW_WINDOW_CMD, SW_MAXIMIZE, SW_RESTORE, ShowWindowAsync,
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
    use objc2::{msg_send, rc::Retained, runtime::AnyObject};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

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
}
