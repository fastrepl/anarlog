#![cfg(target_os = "windows")]

use std::cell::Cell;

use hypr_activity_capture_interface::CaptureError;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

thread_local! {
    static COM_INITIALIZED: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn ensure_com_initialized() -> Result<(), CaptureError> {
    COM_INITIALIZED.with(|initialized| {
        if initialized.get() {
            return Ok(());
        }

        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| CaptureError::platform(error.to_string()))?;
        initialized.set(true);
        Ok(())
    })
}
