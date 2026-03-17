use crossterm::event::KeyEvent;
use hypr_listener2_core::BatchEvent;

use super::runtime::RuntimeEvent;

pub(crate) enum Action {
    Key(KeyEvent),
    Paste(String),
    RuntimeEvent(RuntimeEvent),
    BatchEvent(BatchEvent),
}
