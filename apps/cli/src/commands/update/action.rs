use crossterm::event::KeyEvent;

pub(crate) enum Action {
    Key(KeyEvent),
}
