pub use hypr_activity_capture_interface::*;
pub mod screenshot;
pub use screenshot::*;

#[cfg(target_os = "macos")]
pub type PlatformCapture = hypr_activity_capture_macos::MacosCapture;

#[cfg(target_os = "windows")]
pub type PlatformCapture = hypr_activity_capture_windows::WindowsCapture;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub type PlatformCapture = hypr_activity_capture_macos::MacosCapture;
