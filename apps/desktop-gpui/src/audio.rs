//! The audio provider the Tauri app builds in `create_audio_provider`: the
//! real cpal-backed provider, or `MockAudio` when `MOCK_AUDIO=<n>` is set on a
//! dev/staging build.

use std::sync::Arc;

pub use anlg_audio::AudioProvider;

/// The app-wide provider, installed once in `main`.
pub struct Audio(pub Arc<dyn AudioProvider>);

impl gpui::Global for Audio {}

const STAGING_IDENTIFIER: &str = "com.hyprnote.staging";

pub fn provider(identifier: &str) -> Arc<dyn AudioProvider> {
    let selection: u32 = std::env::var("MOCK_AUDIO")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mock_allowed = cfg!(debug_assertions) || identifier == STAGING_IDENTIFIER;
    if mock_allowed && selection > 0 {
        return Arc::new(anlg_audio_mock::MockAudio::new(selection));
    }
    Arc::new(anlg_audio_actual::ActualAudio)
}

/// `PermissionStatus` as the Linux/Windows checks produce it: the probes
/// either open a stream or fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionStatus {
    Authorized,
    Denied,
}

/// `check_microphone` off macOS: `probe_mic(None)`.
pub fn check_microphone(audio: &dyn AudioProvider) -> PermissionStatus {
    match audio.probe_mic(None) {
        Ok(()) => PermissionStatus::Authorized,
        Err(_) => PermissionStatus::Denied,
    }
}

/// `check_system_audio` off macOS: `probe_speaker()`.
pub fn check_system_audio(audio: &dyn AudioProvider) -> PermissionStatus {
    match audio.probe_speaker() {
        Ok(()) => PermissionStatus::Authorized,
        Err(_) => PermissionStatus::Denied,
    }
}

/// `request_microphone` off macOS: the probe itself, surfacing its error.
pub fn request_microphone(audio: &dyn AudioProvider) -> Result<(), String> {
    audio.probe_mic(None).map_err(|error| error.to_string())
}

/// `request_system_audio`: play silence so the speaker tap has something to
/// open, then probe it.
pub fn request_system_audio(audio: &dyn AudioProvider) -> Result<(), String> {
    let stop = audio.play_silence();
    let result = audio.probe_speaker().map_err(|error| error.to_string());
    let _ = stop.send(());
    result
}
