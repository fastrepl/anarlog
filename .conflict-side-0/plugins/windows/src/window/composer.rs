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

pub const WIDTH: f64 = 720.0;
pub const HEIGHT: f64 = 204.0;

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(ComposerPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true,
        }
    })

    panel_event!(ComposerPanelEventHandler {
        window_did_resign_key(notification: &NSNotification) -> ()
    })
}

#[cfg(target_os = "macos")]
fn panel(app: &AppHandle<tauri::Wry>) -> Result<PanelHandle<tauri::Wry>, Error> {
    app.get_webview_panel(&AppWindow::Composer.label())
        .map_err(|error| Error::PanelError(format!("{error:?}")))
}

#[cfg(target_os = "macos")]
fn create(app: &AppHandle<tauri::Wry>) -> Result<(), Error> {
    let app = app.clone();
    let handle = app.clone();

    run_on_main_thread(&handle, move || {
        let panel = PanelBuilder::<_, ComposerPanel>::new(&app, AppWindow::Composer.label())
            .url(WebviewUrl::App("app/composer".into()))
            .title("Composer")
            .position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
            .size(Size::Logical(LogicalSize::new(WIDTH, HEIGHT)))
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
                    .transparent(true)
                    .background_color(tauri::window::Color(0, 0, 0, 0))
            })
            .style_mask(StyleMask::empty().nonactivating_panel())
            .build()
            .map_err(Error::from)?;

        let handler = ComposerPanelEventHandler::new();
        let handle = app.clone();

        handler.window_did_resign_key(move |_notification| {
            if let Ok(panel) = handle.get_webview_panel(&AppWindow::Composer.label())
                && panel.is_visible()
            {
                panel.hide();
            }
        });

        panel.set_event_handler(Some(handler.as_ref()));

        Ok(())
    })?
}

pub fn ensure(app: &AppHandle<tauri::Wry>) -> Result<(), Error> {
    if app
        .get_webview_window(&AppWindow::Composer.label())
        .is_some()
    {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        create(app)?;
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
                .get_webview_window(&AppWindow::Composer.label())
                .ok_or_else(|| Error::WindowNotFound(AppWindow::Composer.label()))?;
            let panel = panel(&app)?;

            position(&app, &window)?;
            panel.show_and_make_key();

            Ok(window)
        })?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        app.get_webview_window(&AppWindow::Composer.label())
            .ok_or_else(|| Error::WindowNotFound(AppWindow::Composer.label()))
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
        if let Some(window) = app.get_webview_window(&AppWindow::Composer.label()) {
            window.hide()?;
        }

        Ok(())
    }
}

fn position(app: &AppHandle<tauri::Wry>, window: &WebviewWindow<tauri::Wry>) -> Result<(), Error> {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|cursor| app.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or(Error::MonitorNotFound)?;
    let scale_factor = monitor.scale_factor();
    let work_area = monitor.work_area();
    let work_area_size = work_area.size.to_logical::<f64>(scale_factor);
    let work_area_position = work_area.position.to_logical::<f64>(scale_factor);
    let bottom_offset = (work_area_size.height * 0.06).clamp(28.0, 56.0);

    window.set_position(Position::Logical(LogicalPosition::new(
        work_area_position.x + ((work_area_size.width - WIDTH) / 2.0),
        work_area_position.y + work_area_size.height - HEIGHT - bottom_offset,
    )))?;

    Ok(())
}
