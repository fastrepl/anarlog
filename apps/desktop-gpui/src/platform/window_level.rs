//! Keeps a window above other apps' windows (shipping's "Always on Top"). GPUI only sets a
//! window's level when it opens, so the native adapter changes the window level directly.
//! Tests install their own adapter as the [`WindowLevels`] global.

use std::rc::Rc;

use desktop_runtime::{Result, ServiceError};
use gpui::{App, Global, Window};

pub trait WindowLevel {
    fn always_on_top(&self, window: &Window) -> Result<bool>;
    fn set_always_on_top(&self, window: &Window, on: bool) -> Result<()>;
}

#[derive(Clone)]
pub struct WindowLevels(pub Rc<dyn WindowLevel>);

impl Global for WindowLevels {}

pub fn window_level(cx: &App) -> Rc<dyn WindowLevel> {
    cx.try_global::<WindowLevels>()
        .map(|levels| levels.0.clone())
        .unwrap_or_else(|| Rc::new(Native))
}

struct Native;

#[cfg(target_os = "macos")]
mod macos {
    use objc2::{msg_send, runtime::AnyObject};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use super::*;

    /// `NSNormalWindowLevel` and `NSFloatingWindowLevel`, which Tauri's `setAlwaysOnTop` uses.
    const NORMAL: isize = 0;
    const FLOATING: isize = 3;

    fn ns_window(window: &Window) -> Result<*mut AnyObject> {
        let unsupported = || ServiceError::Unsupported("This window has no native handle".into());
        let handle = HasWindowHandle::window_handle(window).map_err(|_| unsupported())?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err(unsupported());
        };
        let view = handle.ns_view.as_ptr().cast::<AnyObject>();
        // SAFETY: the view belongs to this open window.
        let ns_window: *mut AnyObject = unsafe { msg_send![view, window] };
        if ns_window.is_null() {
            return Err(unsupported());
        }
        Ok(ns_window)
    }

    impl WindowLevel for Native {
        fn always_on_top(&self, window: &Window) -> Result<bool> {
            let ns_window = ns_window(window)?;
            // SAFETY: `level` is a plain NSWindow getter.
            let level: isize = unsafe { msg_send![ns_window, level] };
            Ok(level == FLOATING)
        }

        fn set_always_on_top(&self, window: &Window, on: bool) -> Result<()> {
            let ns_window = ns_window(window)?;
            // SAFETY: `setLevel:` accepts any NSWindowLevel on the main thread, where GPUI runs.
            unsafe {
                let _: () = msg_send![ns_window, setLevel: if on { FLOATING } else { NORMAL }];
            }
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl WindowLevel for Native {
    fn always_on_top(&self, _: &Window) -> Result<bool> {
        Err(ServiceError::Unsupported(
            "Always on Top is not available on this platform yet".into(),
        ))
    }

    fn set_always_on_top(&self, _: &Window, _: bool) -> Result<()> {
        Err(ServiceError::Unsupported(
            "Always on Top is not available on this platform yet".into(),
        ))
    }
}
