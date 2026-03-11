use tauri::{Result, image::Image};

pub enum TrayIconState {
    Default,
}

impl TrayIconState {
    pub fn to_image(&self) -> Result<Image<'static>> {
        match self {
            TrayIconState::Default => {
                Image::from_bytes(include_bytes!("../icons/tray_default.png"))
            }
        }
    }
}
