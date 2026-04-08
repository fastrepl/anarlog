#[cfg(target_os = "windows")]
mod capture;
#[cfg(target_os = "windows")]
mod com;
#[cfg(target_os = "windows")]
mod session;

#[cfg(target_os = "windows")]
pub use capture::WindowsCapture;

#[cfg(not(target_os = "windows"))]
use hypr_activity_capture_interface::{
    ActivityCapture, Capabilities, CaptureError, CapturePolicy, CaptureStream, WatchOptions,
};

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Default)]
pub struct WindowsCapture {
    #[allow(dead_code)]
    policy: CapturePolicy,
}

#[cfg(not(target_os = "windows"))]
impl WindowsCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(policy: CapturePolicy) -> Self {
        Self { policy }
    }
}

#[cfg(not(target_os = "windows"))]
impl ActivityCapture for WindowsCapture {
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    fn snapshot(&self) -> Result<Option<hypr_activity_capture_interface::Snapshot>, CaptureError> {
        Err(CaptureError::unsupported(
            "activity-capture-windows is only available on Windows",
        ))
    }

    fn watch(&self, _options: WatchOptions) -> Result<CaptureStream, CaptureError> {
        Err(CaptureError::unsupported(
            "activity-capture-windows is only available on Windows",
        ))
    }
}
