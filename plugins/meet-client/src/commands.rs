use crate::{
    Error, MeetClientPluginExt, MeetMeetingStatus, MeetObservation, MeetSelectors, WINDOW_LABEL,
};

#[tauri::command]
#[specta::specta]
pub(crate) fn is_available() -> bool {
    crate::is_enabled()
}

#[tauri::command]
#[specta::specta]
pub(crate) fn join_meeting<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    meeting_url: String,
) -> Result<String, String> {
    app.meet_client()
        .join(meeting_url)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn leave_meeting<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.meet_client().leave().map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_status<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> MeetMeetingStatus {
    app.meet_client().status()
}

#[tauri::command]
#[specta::specta]
pub(crate) fn set_selectors<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    selectors: MeetSelectors,
) -> Result<(), String> {
    app.meet_client()
        .set_selectors(selectors)
        .map_err(|error| error.to_string())
}

/// Invoked by the observer script running inside the Meet webview.
#[tauri::command]
#[specta::specta]
pub(crate) fn report_observation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    window: tauri::Window<R>,
    observation: MeetObservation,
) -> Result<(), String> {
    if window.label() != WINDOW_LABEL {
        return Err(Error::UnexpectedWindow.to_string());
    }
    app.meet_client()
        .observe(observation)
        .map_err(|error| error.to_string())
}
