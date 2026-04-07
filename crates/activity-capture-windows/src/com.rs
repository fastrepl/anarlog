#![cfg(target_os = "windows")]

use hypr_activity_capture_interface::CaptureError;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

pub(crate) struct ComGuard;

impl ComGuard {
    pub(crate) fn initialize_mta() -> Result<Self, CaptureError> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| CaptureError::platform(error.to_string()))?;

        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
