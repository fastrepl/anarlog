use gpui::{Rgba, rgb};

/// Neutral light palette matching the Tauri app's default appearance. Dark
/// mode and user appearance settings land once settings move off the webview.
#[derive(Clone, Copy)]
pub struct Theme {
    pub background: Rgba,
    pub sidebar: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub selected: Rgba,
    pub hover: Rgba,
    pub danger: Rgba,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            background: rgb(0xffffff),
            sidebar: rgb(0xf5f5f5),
            border: rgb(0xe5e5e5),
            text: rgb(0x171717),
            text_muted: rgb(0x737373),
            selected: rgb(0xe5e5e5),
            hover: rgb(0xebebeb),
            danger: rgb(0xb91c1c),
        }
    }
}
