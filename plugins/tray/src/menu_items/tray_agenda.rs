use tauri::{
    AppHandle, Result,
    menu::{MenuId, MenuItem, MenuItemKind},
};
use tauri_plugin_windows::{AppWindow, Navigate, WindowsPluginExt};

use crate::ext::scheduled_event;

const ID_PREFIX: &str = "anlg_tray_agenda_";

pub fn build_agenda_item(
    app: &AppHandle<tauri::Wry>,
    event_id: &str,
    text: &str,
) -> Result<MenuItemKind<tauri::Wry>> {
    let item = MenuItem::with_id(
        app,
        format!("{ID_PREFIX}{event_id}"),
        text,
        true,
        None::<&str>,
    )?;
    Ok(MenuItemKind::MenuItem(item))
}

pub fn handle_agenda_menu_event(app: &AppHandle<tauri::Wry>, id: &MenuId) -> bool {
    let Some(event_id) = id.0.strip_prefix(ID_PREFIX) else {
        return false;
    };

    let Some(event) = scheduled_event(event_id) else {
        return true;
    };

    if app.windows().show(AppWindow::Main).is_ok() {
        let _ = app
            .windows()
            .emit_navigate(AppWindow::Main, event_navigation(event.id));
    }

    if let Some(meeting_link) = event.meeting_link.filter(|link| !link.trim().is_empty())
        && let Err(error) = open::that(meeting_link)
    {
        tracing::warn!(%error, "failed to open meeting from tray agenda");
    }

    true
}

fn event_navigation(event_id: String) -> Navigate {
    let mut search = serde_json::Map::new();
    search.insert(
        "calendarEventId".to_string(),
        serde_json::Value::String(event_id),
    );
    search.insert(
        "record".to_string(),
        serde_json::Value::String("true".to_string()),
    );

    Navigate {
        path: "/app/new".to_string(),
        search: Some(search),
    }
}

#[cfg(test)]
mod tests {
    use super::event_navigation;

    #[test]
    fn opens_the_selected_calendar_event_with_recording_enabled() {
        let navigation = event_navigation("event-123".to_string());
        let search = navigation.search.unwrap();

        assert_eq!(navigation.path, "/app/new");
        assert_eq!(search["calendarEventId"], "event-123");
        assert_eq!(search["record"], "true");
    }
}
