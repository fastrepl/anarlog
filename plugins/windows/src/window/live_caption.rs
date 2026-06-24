use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LiveCaptionPosition {
    TopCenter,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    BottomCenter,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LiveCaptionState {
    pub text: String,
    pub opacity: f64,
    pub width: f64,
    pub line_count: u32,
    pub position: LiveCaptionPosition,
    pub minimized: bool,
}

#[cfg(target_os = "macos")]
mod platform {
    use swift_rs::{Bool, SRString, swift};

    use super::LiveCaptionState;
    use crate::Error;

    swift!(fn _live_caption_show() -> Bool);
    swift!(fn _live_caption_hide() -> Bool);
    swift!(fn _live_caption_update(json: &SRString) -> Bool);

    pub fn show() -> Result<(), Error> {
        unsafe {
            _live_caption_show();
        }
        Ok(())
    }

    pub fn hide() -> Result<(), Error> {
        unsafe {
            _live_caption_hide();
        }
        Ok(())
    }

    pub fn update(state: LiveCaptionState) -> Result<(), Error> {
        let json = serde_json::to_string(&state).map_err(|error| {
            Error::PanelError(format!("failed to serialize live caption state: {error}"))
        })?;
        let json = SRString::from(json.as_str());

        let ok = unsafe { _live_caption_update(&json) };
        if ok {
            Ok(())
        } else {
            Err(Error::PanelError(
                "failed to update native live caption".to_string(),
            ))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::LiveCaptionState;
    use crate::Error;

    pub fn show() -> Result<(), Error> {
        Ok(())
    }

    pub fn hide() -> Result<(), Error> {
        Ok(())
    }

    pub fn update(_state: LiveCaptionState) -> Result<(), Error> {
        Ok(())
    }
}

pub fn show() -> Result<(), Error> {
    platform::show()
}

pub fn hide() -> Result<(), Error> {
    platform::hide()
}

pub fn update(state: LiveCaptionState) -> Result<(), Error> {
    platform::update(state)
}
