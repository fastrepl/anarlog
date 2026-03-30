use tauri_specta::Event as _;

use hypr_calendar::runtime::CalendarRuntime;
use hypr_calendar_worker::runtime::{NotificationWorkerEvent, NotificationWorkerRuntime};

use crate::events::CalendarChangedEvent;

pub struct TauriCalendarRuntime<R: tauri::Runtime>(pub tauri::AppHandle<R>);

impl<R: tauri::Runtime> CalendarRuntime for TauriCalendarRuntime<R> {
    fn emit_changed(&self) {
        let _ = CalendarChangedEvent.emit(&self.0);
    }
}

pub struct TauriNotificationWorkerRuntime<R: tauri::Runtime>(pub tauri::AppHandle<R>);

impl<R: tauri::Runtime> NotificationWorkerRuntime for TauriNotificationWorkerRuntime<R> {
    fn emit(&self, event: NotificationWorkerEvent) {
        let _ = event.emit(&self.0);
    }
}
