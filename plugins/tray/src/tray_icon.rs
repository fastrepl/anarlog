use tauri::{Result, image::Image};

pub enum TrayIconState {
    Default,
    UpdateAvailable,
}

pub const RECORDING_FRAMES: &[&[u8]] = &[
    include_bytes!("../icons/tray_recording_0.png"),
    include_bytes!("../icons/tray_recording_1.png"),
    include_bytes!("../icons/tray_recording_2.png"),
    include_bytes!("../icons/tray_recording_3.png"),
];

impl TrayIconState {
    pub fn to_image(&self) -> Result<Image<'static>> {
        match self {
            TrayIconState::Default => {
                Image::from_bytes(include_bytes!("../icons/tray_default.png"))
            }
            TrayIconState::UpdateAvailable => {
                Image::from_bytes(include_bytes!("../icons/tray_update.png"))
            }
        }
    }
}
