use crossterm::event::KeyEvent;

use super::list::ModelRow;

pub(crate) enum Action {
    Key(KeyEvent),
    Loaded(Vec<ModelRow>),
    LoadError(String),
}
