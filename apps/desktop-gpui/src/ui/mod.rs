pub mod assets;
pub mod caret;
pub mod input;
pub mod text;
pub mod theme;

use gpui::{App, FocusHandle, Window};

#[derive(Clone)]
pub struct FocusReturn(pub FocusHandle);

impl FocusReturn {
    pub fn restore(&self, window: &mut Window, cx: &mut App) {
        self.0.focus(window, cx);
    }
}
