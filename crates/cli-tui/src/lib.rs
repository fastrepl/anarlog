mod event;
mod frame;
mod render;
mod screen;
mod terminal;

pub use event::{EventHandler, TuiEvent};
pub use frame::FrameRequester;
pub use render::render_inline;
pub use screen::{
    Screen, ScreenContext, ScreenControl, run_screen, run_screen_inline, terminal_title,
};
pub use terminal::TerminalGuard;
