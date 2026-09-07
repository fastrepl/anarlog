use anlg_tray_core::icons;
use tauri::{Result, image::Image};

pub enum TrayIconState {
    Default,
    Degraded,
    UpdateAvailable,
}

pub const RECORDING_FRAMES: &[&[u8]] = icons::RECORDING_FRAMES;

impl TrayIconState {
    pub fn to_image(&self) -> Result<Image<'static>> {
        match self {
            TrayIconState::Default => Image::from_bytes(icons::DEFAULT),
            TrayIconState::Degraded => Image::from_bytes(icons::DEGRADED),
            TrayIconState::UpdateAvailable => Image::from_bytes(icons::UPDATE_AVAILABLE),
        }
    }
}
