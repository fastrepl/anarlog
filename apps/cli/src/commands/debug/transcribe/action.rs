use crossterm::event::KeyEvent;

use super::runtime::RuntimeEvent;

pub(crate) enum Action {
    Key(KeyEvent),
    Runtime(RuntimeEvent),
}
