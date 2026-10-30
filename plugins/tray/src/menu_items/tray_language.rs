use tauri::{
    AppHandle, Result,
    menu::{CheckMenuItem, MenuId, Submenu},
};
use tauri_specta::Event;

use crate::{TrayLanguageOption, TrayLanguageSelected};

const ID_PREFIX: &str = "anlg_tray_language_";

pub fn build_language_submenu(
    app: &AppHandle<tauri::Wry>,
    options: &[TrayLanguageOption],
) -> Result<Option<Submenu<tauri::Wry>>> {
    if options.len() < 2 {
        return Ok(None);
    }

    let submenu = Submenu::new(app, "Transcription language", true)?;
    for option in options {
        let item = CheckMenuItem::with_id(
            app,
            language_item_id(&option.code),
            &option.label,
            true,
            option.selected,
            None::<&str>,
        )?;
        submenu.append(&item)?;
    }

    Ok(Some(submenu))
}

pub fn handle_language_menu_event(app: &AppHandle<tauri::Wry>, id: &MenuId) -> bool {
    let Some(code) = language_code(id) else {
        return false;
    };

    let event = TrayLanguageSelected {
        code: code.to_string(),
    };
    if let Err(error) = event.emit(app) {
        tracing::warn!(%error, "failed to emit tray language selection");
    }

    true
}

fn language_item_id(code: &str) -> String {
    format!("{ID_PREFIX}{code}")
}

fn language_code(id: &MenuId) -> Option<&str> {
    id.0.strip_prefix(ID_PREFIX).filter(|code| !code.is_empty())
}
