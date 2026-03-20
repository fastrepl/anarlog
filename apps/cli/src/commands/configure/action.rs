use crossterm::event::KeyEvent;

use super::runtime::RuntimeEvent;

pub enum Action {
    Key(KeyEvent),
    Runtime(RuntimeEvent),
}
