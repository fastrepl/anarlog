use hypr_calendar::runtime::CalendarRuntime;
use tauri_specta::Event as _;

use crate::events::CalendarChangedEvent;

pub struct TauriCalendarRuntime<R: tauri::Runtime>(pub tauri::AppHandle<R>);

impl<R: tauri::Runtime> CalendarRuntime for TauriCalendarRuntime<R> {
    fn emit_changed(&self) {
        let _ = CalendarChangedEvent.emit(&self.0);
    }
}
