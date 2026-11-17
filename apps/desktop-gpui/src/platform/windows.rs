static RESTART: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn request_restart() {
    RESTART.store(true, std::sync::atomic::Ordering::Release);
}

pub fn restart_requested() -> bool {
    RESTART.load(std::sync::atomic::Ordering::Acquire)
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
                    traffic_light_position: matches!(self, Self::Main)
                        .then(|| point(px(12.), px(16.))),
                    ..Default::default()
                })
            },
            app_id: Some("com.anarlog.gpui.sandbox".into()),
            ..Default::default()
        })
    }
}

/// Hides only the main window like shipping's `AppWindow::Main.hide` (`orderOut:`), leaving
/// note windows open; the next show or Dock click brings it back.
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
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cx;
        Err(ServiceError::Unsupported(
            "This platform cannot hide a single window".into(),
        ))
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
