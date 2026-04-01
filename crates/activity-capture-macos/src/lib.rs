#[cfg(target_os = "macos")]
mod app_profile;
#[cfg(target_os = "macos")]
mod apple_script;
#[cfg(target_os = "macos")]
mod ax;
#[cfg(target_os = "macos")]
mod browser_url;
#[cfg(target_os = "macos")]
mod handlers;
#[cfg(target_os = "macos")]
mod platform;
#[cfg(target_os = "macos")]
mod runtime;
#[cfg(target_os = "macos")]
mod sanitize;
#[cfg(target_os = "macos")]
mod slack;
#[cfg(target_os = "macos")]
mod spotify;

#[cfg(target_os = "macos")]
pub use platform::MacosCapture;

#[cfg(not(target_os = "macos"))]
use hypr_activity_capture_interface::{
    ActivityCapture, Capabilities, CaptureError, CapturePolicy, CaptureStream, WatchOptions,
};

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Clone, Default)]
pub struct MacosCapture {
    #[allow(dead_code)]
    policy: CapturePolicy,
}

#[cfg(not(target_os = "macos"))]
impl MacosCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(policy: CapturePolicy) -> Self {
        Self { policy }
    }
}

#[cfg(not(target_os = "macos"))]
impl ActivityCapture for MacosCapture {
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    fn snapshot(&self) -> Result<Option<hypr_activity_capture_interface::Snapshot>, CaptureError> {
        Err(CaptureError::unsupported(
            "activity-capture-macos is only available on macOS",
        ))
    }

    fn watch(&self, _options: WatchOptions) -> Result<CaptureStream, CaptureError> {
        Err(CaptureError::unsupported(
            "activity-capture-macos is only available on macOS",
        ))
    }
}
