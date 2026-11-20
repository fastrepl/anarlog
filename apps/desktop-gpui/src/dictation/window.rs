//! Where the dictation panel opens: like shipping's `FloatingBarManager` in dictation mode,
//! bottom center of the screen under the pointer, 8px above its edge, without taking focus
//! from the field being dictated into.

use gpui::{
    App, Bounds, Pixels, Size, Window, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions, point, px,
};

const SCREEN_MARGIN: f32 = 8.;

pub fn bottom_center(visible: Bounds<Pixels>, size: Size<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(
            visible.center().x - size.width / 2.,
            visible.bottom() - size.height - px(SCREEN_MARGIN),
        ),
        size,
    )
}

pub fn options(size: Size<Pixels>, cx: &App) -> WindowOptions {
    let display = pointer_display(cx).or_else(|| cx.primary_display());
    let bounds = match &display {
        Some(display) => bottom_center(display.visible_bounds(), size),
        None => Bounds::centered(None, size, cx),
    };
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: display.map(|display| display.id()),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        app_id: crate::platform::package::window_app_id(),
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
fn pointer_display(cx: &App) -> Option<std::rc::Rc<dyn gpui::PlatformDisplay>> {
    use std::ffi::c_void;

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreate(source: *const c_void) -> *mut c_void;
        fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
        fn CGGetDisplaysWithPoint(
            point: CGPoint,
            max: u32,
            displays: *mut u32,
            count: *mut u32,
        ) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: *const c_void);
    }

    // SAFETY: a null-source event only reads the current pointer location; it is released here.
    let location = unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return None;
        }
        let location = CGEventGetLocation(event);
        CFRelease(event);
        location
    };
    let mut display = 0_u32;
    let mut count = 0_u32;
    // SAFETY: room for exactly one display id is provided.
    let status = unsafe { CGGetDisplaysWithPoint(location, 1, &mut display, &mut count) };
    (status == 0 && count == 1)
        .then(|| cx.find_display(gpui::DisplayId::new(u64::from(display))))
        .flatten()
}

#[cfg(not(target_os = "macos"))]
fn pointer_display(_: &App) -> Option<std::rc::Rc<dyn gpui::PlatformDisplay>> {
    None
}

/// Clicks on the panel must not make it key, or the dictated app would lose its focused field
/// (shipping's `FloatingBarPanel.canBecomeKey` is false while dictating).
#[cfg(target_os = "macos")]
pub fn keep_focus_elsewhere(window: &Window) {
    use objc2::{msg_send, runtime::AnyObject, runtime::Bool};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    let view = handle.ns_view.as_ptr().cast::<AnyObject>();
    // SAFETY: the view belongs to this open window; NSPanel accepts this setter.
    unsafe {
        let panel: *mut AnyObject = msg_send![view, window];
        if !panel.is_null() {
            let _: () = msg_send![panel, setBecomesKeyOnlyIfNeeded: Bool::YES];
        }
    }
}

#[cfg(target_os = "windows")]
pub fn keep_focus_elsewhere(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                GWL_EXSTYLE, GetWindowLongPtrW, MA_NOACTIVATE, SetWindowLongPtrW, WM_MOUSEACTIVATE,
                WM_NCDESTROY, WS_EX_NOACTIVATE,
            },
        },
    };

    unsafe extern "system" fn panel_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        id: usize,
        _: usize,
    ) -> LRESULT {
        // GPUI otherwise returns MA_ACTIVATE even on a WS_EX_NOACTIVATE window.
        // MA_NOACTIVATE preserves the external field's focus and delivers the click.
        if message == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        // SAFETY: comctl32 calls this on the window thread with its current message.
        unsafe {
            if message == WM_NCDESTROY {
                let _ = RemoveWindowSubclass(hwnd, Some(panel_proc), id);
            }
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
    }

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    // SAFETY: this is the overlay's live HWND on its UI thread. Preserve GPUI's other styles.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as isize);
        if !SetWindowSubclass(hwnd, Some(panel_proc), 0, 0).as_bool() {
            tracing::warn!("Could not prevent dictation panel clicks from taking focus");
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn keep_focus_elsewhere(_: &Window) {}
