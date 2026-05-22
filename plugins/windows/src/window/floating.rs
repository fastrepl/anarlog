use tauri::{AppHandle, LogicalPosition, Manager, Position, WebviewWindow};

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    CollectionBehavior, ManagerExt, PanelBuilder, PanelHandle, PanelLevel, StyleMask, tauri_panel,
};

#[cfg(target_os = "macos")]
use tauri::{LogicalSize, Size, WebviewUrl};

#[cfg(target_os = "macos")]
use crate::ext::run_on_main_thread;
use crate::{AppWindow, Error, WindowImpl};

pub const COLLAPSED_WIDTH: f64 = 124.0;
pub const COLLAPSED_HEIGHT: f64 = 48.0;

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(FloatingPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true,
        }
    })
}

#[cfg(target_os = "macos")]
fn panel(app: &AppHandle<tauri::Wry>) -> Result<PanelHandle<tauri::Wry>, Error> {
    app.get_webview_panel(&AppWindow::Floating.label())
        .map_err(|error| Error::PanelError(format!("{error:?}")))
}

#[cfg(target_os = "macos")]
fn create(app: &AppHandle<tauri::Wry>) -> Result<(), Error> {
    let app = app.clone();
    let handle = app.clone();

    run_on_main_thread(&handle, move || {
        PanelBuilder::<_, FloatingPanel>::new(&app, AppWindow::Floating.label())
            .url(WebviewUrl::App("floating".into()))
            .title("Meeting Float")
            .position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
            .size(Size::Logical(LogicalSize::new(
                COLLAPSED_WIDTH,
                COLLAPSED_HEIGHT,
            )))
            .level(PanelLevel::Floating)
            .has_shadow(false)
            .collection_behavior(
                CollectionBehavior::new()
                    .full_screen_auxiliary()
                    .can_join_all_spaces(),
            )
            .hides_on_deactivate(false)
            .works_when_modal(true)
            .no_activate(true)
            .with_window(|window| {
                window
                    .visible(false)
                    .decorations(false)
                    .resizable(false)
                    .transparent(true)
                    .background_color(tauri::window::Color(0, 0, 0, 0))
            })
            .style_mask(StyleMask::empty().nonactivating_panel())
            .build()
            .map_err(Error::from)?;

        Ok(())
    })?
}

pub fn ensure(app: &AppHandle<tauri::Wry>) -> Result<(), Error> {
    if app
        .get_webview_window(&AppWindow::Floating.label())
        .is_some()
    {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        create(app)?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        AppWindow::Floating.build_window(app)?;
    }

    Ok(())
}

pub fn show(app: &AppHandle<tauri::Wry>) -> Result<WebviewWindow, Error> {
    ensure(app)?;

    #[cfg(target_os = "macos")]
    {
        let app = app.clone();
        let handle = app.clone();

        return run_on_main_thread(&handle, move || {
            let window = app
                .get_webview_window(&AppWindow::Floating.label())
                .ok_or_else(|| Error::WindowNotFound(AppWindow::Floating.label()))?;
            let panel = panel(&app)?;

            position(&app, &window)?;
            panel.show();

            Ok(window)
        })?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let window = app
            .get_webview_window(&AppWindow::Floating.label())
            .ok_or_else(|| Error::WindowNotFound(AppWindow::Floating.label()))?;
        window.show()?;
        Ok(window)
    }
}

pub fn hide(app: &AppHandle<tauri::Wry>) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        ensure(app)?;
        let app = app.clone();
        let handle = app.clone();

        return run_on_main_thread(&handle, move || {
            panel(&app)?.hide();
            Ok(())
        })?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app.get_webview_window(&AppWindow::Floating.label()) {
            window.hide()?;
        }

        Ok(())
    }
}

fn position(app: &AppHandle<tauri::Wry>, window: &WebviewWindow<tauri::Wry>) -> Result<(), Error> {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or(Error::MonitorNotFound)?;
    let scale_factor = monitor.scale_factor();
    let work_area = monitor.work_area();
    let work_area_size = work_area.size.to_logical::<f64>(scale_factor);
    let work_area_position = work_area.position.to_logical::<f64>(scale_factor);
    let bottom_offset = (work_area_size.height * 0.12).clamp(56.0, 120.0);
    let right_offset = 32.0;

    window.set_position(Position::Logical(LogicalPosition::new(
        work_area_position.x + work_area_size.width - COLLAPSED_WIDTH - right_offset,
        work_area_position.y + work_area_size.height - COLLAPSED_HEIGHT - bottom_offset,
    )))?;

    Ok(())
}
