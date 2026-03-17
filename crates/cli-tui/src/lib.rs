mod event;
mod frame;
mod screen;
mod terminal;
mod textarea_input;

pub use event::{EventHandler, TuiEvent};
pub use frame::FrameRequester;
pub use screen::{Screen, ScreenContext, ScreenControl, run_screen, run_screen_inline};
pub use terminal::TerminalGuard;
pub use textarea_input::textarea_input_from_key_event;
