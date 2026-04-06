#![cfg(target_os = "macos")]

use hypr_activity_capture_interface::{
    CaptureError, CaptureStream, WatchOptions, spawn_polling_watch_stream,
};

use crate::platform::MacosCapture;

pub(crate) fn spawn_watch_stream(
    capture: MacosCapture,
    options: WatchOptions,
) -> Result<CaptureStream, CaptureError> {
    spawn_polling_watch_stream(
        "activity-capture-macos",
        move || capture.capture_snapshot(),
        options,
    )
}
