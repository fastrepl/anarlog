use crate::{
    AppWindow, SavedFrames, SavedWindowFrame, WebviewHealthState, WindowImpl, WindowsPluginExt,
    events,
};

use tauri::Manager;

#[tauri::command]
#[specta::specta]
pub async fn window_show(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    app.windows()
        .show_async(window)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_hide(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    app.windows().hide(window).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn webview_health_ack(
    window: tauri::Window<tauri::Wry>,
    request_id: String,
) -> Result<(), String> {
    if let Some(state) = window.try_state::<WebviewHealthState>() {
        state.acknowledge(window.label(), &request_id);
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn webview_health_ready(window: tauri::Window<tauri::Wry>) -> Result<(), String> {
    if let Some(state) = window.try_state::<WebviewHealthState>() {
        state.ready(window.label());
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_destroy(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    app.windows().destroy(window).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_show_app_in_dock(
    app: tauri::AppHandle<tauri::Wry>,
    show: bool,
) -> Result<(), String> {
    app.windows()
        .set_show_app_in_dock(show)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_navigate(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
    path: String,
) -> Result<(), String> {
    app.windows()
        .navigate(window, path)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_emit_navigate(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
    event: events::Navigate,
) -> Result<(), String> {
    app.windows()
        .emit_navigate(window, event)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Deserialize, specta::Type)]
pub enum Anchor {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
    Center,
}

#[tauri::command]
#[specta::specta]
pub async fn window_set_frame_animated(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
    anchor: Anchor,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if matches!(window, AppWindow::Main)
        && let Some(window_handle) = window.get(&app)
    {
        if window_handle.is_maximized().map_err(|e| e.to_string())? {
            window_handle.unmaximize().map_err(|e| e.to_string())?;
        }
        window_handle
            .set_always_on_top(true)
            .map_err(|e| e.to_string())?;
    }

    let visible_frame = app
        .windows()
        .visible_frame(window.clone())
        .map_err(|e| e.to_string())?;

    if let Some(screen) = visible_frame {
        let frame = anchored_frame(anchor, screen, width, height);

        app.windows()
            .set_frame_animated(window, frame)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

const ANCHOR_MARGIN: f64 = 8.0;

fn anchored_frame(
    anchor: Anchor,
    screen: crate::SavedFrame,
    width: f64,
    height: f64,
) -> crate::SavedFrame {
    let left = screen.x + ANCHOR_MARGIN;
    let right = screen.x + screen.w - width - ANCHOR_MARGIN;
    let center_x = screen.x + (screen.w - width) / 2.0;

    let (x, offset_from_top) = match anchor {
        Anchor::TopRight => (right, ANCHOR_MARGIN),
        Anchor::TopLeft => (left, ANCHOR_MARGIN),
        Anchor::BottomRight => (right, screen.h - height - ANCHOR_MARGIN),
        Anchor::BottomLeft => (left, screen.h - height - ANCHOR_MARGIN),
        Anchor::Center => (center_x, (screen.h - height) / 2.0),
    };

    let y = if cfg!(target_os = "macos") {
        screen.y + screen.h - height - offset_from_top
    } else {
        screen.y + offset_from_top
    };

    crate::SavedFrame {
        x,
        y,
        w: width,
        h: height,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn window_save_frame(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    let maximized = if let Some(handle) = window.get(&app) {
        let maximized = handle.is_maximized().map_err(|e| e.to_string())?;
        if maximized {
            #[cfg(target_os = "linux")]
            let previous_size = handle.inner_size().map_err(|e| e.to_string())?;
            handle.unmaximize().map_err(|e| e.to_string())?;
            #[cfg(target_os = "linux")]
            let restored = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                loop {
                    if !handle.is_maximized().map_err(|e| e.to_string())?
                        && handle.inner_size().map_err(|e| e.to_string())? != previous_size
                    {
                        return Ok::<(), String>(());
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(16)).await;
                }
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|result| result);
            #[cfg(target_os = "linux")]
            if let Err(error) = restored {
                let _ = handle.maximize();
                return Err(error);
            }
        }
        maximized
    } else {
        false
    };
    let frame = app
        .windows()
        .frame(window.clone())
        .map_err(|e| e.to_string())?;

    if let Some(frame) = frame {
        app.state::<SavedFrames>()
            .0
            .lock()
            .unwrap()
            .insert(window.label(), SavedWindowFrame { frame, maximized });
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_restore_frame_animated(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    let saved = app.state::<SavedFrames>().take(&window.label());

    restore_saved_frame(&app, window, saved)
}

pub(crate) fn restore_saved_frame(
    app: &tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
    saved: Option<SavedWindowFrame>,
) -> Result<(), String> {
    if let Some(saved) = saved {
        if let Some(handle) = window.get(app)
            && handle.is_maximized().map_err(|e| e.to_string())?
        {
            handle.unmaximize().map_err(|e| e.to_string())?;
        }
        app.windows()
            .set_frame_animated(window.clone(), saved.frame)
            .map_err(|e| e.to_string())?;
        if saved.maximized
            && let Some(handle) = window.get(app)
        {
            handle.maximize().map_err(|e| e.to_string())?;
        }
    }

    if matches!(window, AppWindow::Main)
        && let Some(window_handle) = window.get(&app)
    {
        window_handle
            .set_always_on_top(false)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_expand_width(
    app: tauri::AppHandle<tauri::Wry>,
    window: tauri::Window<tauri::Wry>,
    expansion_px: u32,
    max_current_width: Option<u32>,
    check_monitor_space: bool,
    expand_left: bool,
    restore_on_close: bool,
) -> Result<(), String> {
    if check_monitor_space {
        let outer_size = window.outer_size().map_err(|e| e.to_string())?;
        let outer_position = window.outer_position().map_err(|e| e.to_string())?;
        let monitor = window.current_monitor().map_err(|e| e.to_string())?;

        if let Some(monitor) = monitor {
            let window_right = i64::from(outer_position.x) + i64::from(outer_size.width);
            let monitor_right = i64::from(monitor.position().x) + i64::from(monitor.size().width);

            if monitor_right - window_right < i64::from(expansion_px) {
                return Ok(());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use crate::ext::run_on_main_thread;
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        let expansion = f64::from(expansion_px);
        let label = window.label().to_string();
        let app_clone = app.clone();

        let pushed = run_on_main_thread(&app, move || {
            let Some(win) = app_clone.get_webview_window(&label) else {
                return None;
            };
            let Ok(ns_win_ptr) = win.ns_window() else {
                return None;
            };
            let ns_window = unsafe { &*(ns_win_ptr as *mut objc2_app_kit::NSWindow) };

            let frame = ns_window.frame();

            if max_current_width.is_some_and(|max| frame.size.width >= f64::from(max)) {
                return None;
            }

            let new_width = frame.size.width + expansion;
            let new_origin_x = if expand_left {
                frame.origin.x - expansion
            } else {
                frame.origin.x
            };
            ns_window.setFrame_display(
                NSRect::new(
                    NSPoint::new(new_origin_x, frame.origin.y),
                    NSSize::new(new_width, frame.size.height),
                ),
                false,
            );
            Some((frame.size.width, new_width, expand_left))
        })
        .map_err(|e| e.to_string())?;

        if restore_on_close && let Some(entry) = pushed {
            app.state::<crate::WindowExpansions>()
                .0
                .lock()
                .unwrap()
                .entry(window.label().to_string())
                .or_default()
                .push(entry);
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let outer_size = window.outer_size().map_err(|e| e.to_string())?;

        if max_current_width.is_some_and(|max| outer_size.width >= max) {
            return Ok(());
        }

        let new_width = outer_size.width + expansion_px;
        window
            .set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: new_width,
                height: outer_size.height,
            }))
            .map_err(|e| e.to_string())?;

        if restore_on_close {
            app.state::<crate::WindowExpansions>()
                .0
                .lock()
                .unwrap()
                .entry(window.label().to_string())
                .or_default()
                .push((
                    f64::from(outer_size.width),
                    f64::from(new_width),
                    expand_left,
                ));
        }
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_restore_width(
    app: tauri::AppHandle<tauri::Wry>,
    window: tauri::Window<tauri::Wry>,
) -> Result<(), String> {
    let entry = app.state::<crate::WindowExpansions>().pop(window.label());

    let Some((previous_w, expanded_w, expand_left)) = entry else {
        return Ok(());
    };

    #[cfg(target_os = "macos")]
    {
        use crate::ext::run_on_main_thread;
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        let label = window.label().to_string();
        let app_clone = app.clone();

        run_on_main_thread(&app, move || {
            let Some(win) = app_clone.get_webview_window(&label) else {
                return;
            };
            let Ok(ns_win_ptr) = win.ns_window() else {
                return;
            };
            let ns_window = unsafe { &*(ns_win_ptr as *mut objc2_app_kit::NSWindow) };

            let frame = ns_window.frame();
            if (frame.size.width - expanded_w).abs() < 1.0 {
                let restore_origin_x = if expand_left {
                    frame.origin.x + (expanded_w - previous_w)
                } else {
                    frame.origin.x
                };
                ns_window.setFrame_display(
                    NSRect::new(
                        NSPoint::new(restore_origin_x, frame.origin.y),
                        NSSize::new(previous_w, frame.size.height),
                    ),
                    false,
                );
            }
        })
        .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let outer_size = window.outer_size().map_err(|e| e.to_string())?;
        if (f64::from(outer_size.width) - expanded_w).abs() < 1.0 {
            window
                .set_size(tauri::Size::Physical(tauri::PhysicalSize {
                    width: previous_w as u32,
                    height: outer_size.height,
                }))
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn floating_bar_show() -> Result<(), String> {
    crate::window::floating_bar::show().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn floating_bar_hide() -> Result<(), String> {
    crate::window::floating_bar::hide().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn floating_bar_update(
    state: crate::window::floating_bar::FloatingBarState,
) -> Result<(), String> {
    crate::window::floating_bar::update(state).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn floating_bar_update_amplitude(amplitude: f64) -> Result<(), String> {
    crate::window::floating_bar::update_amplitude(amplitude).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn floating_bar_current_state()
-> Result<Option<crate::window::floating_bar::FloatingBarState>, String> {
    Ok(crate::window::floating_bar::current_state())
}

#[tauri::command]
#[specta::specta]
pub async fn live_caption_show() -> Result<(), String> {
    crate::window::live_caption::show().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn live_caption_hide() -> Result<(), String> {
    crate::window::live_caption::hide().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn live_caption_update(
    state: crate::window::live_caption::LiveCaptionState,
) -> Result<(), String> {
    crate::window::live_caption::update(state).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn live_caption_current_state()
-> Result<Option<crate::window::live_caption::LiveCaptionState>, String> {
    Ok(crate::window::live_caption::current_state())
}

#[tauri::command]
#[specta::specta]
pub async fn window_is_exists(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<bool, String> {
    let exists = app.windows().is_exists(window).map_err(|e| e.to_string())?;
    Ok(exists)
}

#[tauri::command]
#[specta::specta]
pub async fn window_is_occluded(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<bool, String> {
    let occluded = app
        .windows()
        .is_occluded(window)
        .map_err(|e| e.to_string())?;
    Ok(occluded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> crate::SavedFrame {
        crate::SavedFrame {
            x: 100.0,
            y: 50.0,
            w: 1000.0,
            h: 800.0,
        }
    }

    #[test]
    fn top_right_frame_hugs_the_top_right_corner() {
        let frame = anchored_frame(Anchor::TopRight, screen(), 340.0, 500.0);

        assert_eq!(frame.x, 100.0 + 1000.0 - 340.0 - ANCHOR_MARGIN);
        assert_eq!((frame.w, frame.h), (340.0, 500.0));

        if cfg!(target_os = "macos") {
            assert_eq!(frame.y, 50.0 + 800.0 - 500.0 - ANCHOR_MARGIN);
        } else {
            assert_eq!(frame.y, 50.0 + ANCHOR_MARGIN);
        }
    }

    #[test]
    fn bottom_left_frame_hugs_the_bottom_left_corner() {
        let frame = anchored_frame(Anchor::BottomLeft, screen(), 340.0, 500.0);

        assert_eq!(frame.x, 100.0 + ANCHOR_MARGIN);

        if cfg!(target_os = "macos") {
            assert_eq!(frame.y, 50.0 + ANCHOR_MARGIN);
        } else {
            assert_eq!(frame.y, 50.0 + 800.0 - 500.0 - ANCHOR_MARGIN);
        }
    }
}
