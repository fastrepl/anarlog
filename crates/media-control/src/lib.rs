#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(String);

impl Error {
    pub(crate) fn new(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

/// Pauses media playback in other applications, best effort.
///
/// - macOS: MediaRemote (pre-15.4, where it still works for unprivileged
///   processes) plus AppleScript for scriptable players such as Spotify.
/// - Windows: GlobalSystemMediaTransportControls sessions.
/// - Linux: MPRIS players over D-Bus.
pub async fn pause_playback() -> Result<(), Error> {
    platform::pause_playback().await
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod platform {
    pub(super) async fn pause_playback() -> Result<(), crate::Error> {
        Ok(())
    }
}
