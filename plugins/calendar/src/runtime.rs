use hypr_calendar::runtime::CalendarRuntime;
use tauri::Manager;
use tauri_specta::Event as _;

use crate::events::{CalendarChangedEvent, CalendarSyncEvent};

pub struct TauriCalendarRuntime<R: tauri::Runtime>(pub tauri::AppHandle<R>);
pub struct TauriCalendarSyncRuntime<R: tauri::Runtime>(pub tauri::AppHandle<R>);

impl<R: tauri::Runtime> CalendarRuntime for TauriCalendarRuntime<R> {
    fn emit_changed(&self) {
        let _ = CalendarChangedEvent.emit(&self.0);
        if let Some(state) = self.0.try_state::<crate::CalendarSyncState>() {
            state
                .0
                .request_sync(hypr_calendar_sync::SyncReason::AppleCalendarChanged);
        }
    }
}

impl<R: tauri::Runtime> hypr_calendar_sync::CalendarSyncRuntime for TauriCalendarSyncRuntime<R> {
    fn emit(&self, event: hypr_calendar_sync::CalendarSyncWorkerEvent) {
        let event = match event {
            hypr_calendar_sync::CalendarSyncWorkerEvent::StatusChanged { status } => {
                CalendarSyncEvent::StatusChanged { status }
            }
            hypr_calendar_sync::CalendarSyncWorkerEvent::SyncStarted { reasons } => {
                CalendarSyncEvent::SyncStarted { reasons }
            }
            hypr_calendar_sync::CalendarSyncWorkerEvent::SyncFinished {
                reasons,
                data_changed,
            } => CalendarSyncEvent::SyncFinished {
                reasons,
                data_changed,
            },
            hypr_calendar_sync::CalendarSyncWorkerEvent::SyncFailed { reasons, error } => {
                CalendarSyncEvent::SyncFailed { reasons, error }
            }
        };
        let _ = event.emit(&self.0);
    }
}
