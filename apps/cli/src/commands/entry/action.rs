use crossterm::event::KeyEvent;

pub(crate) enum Action {
    Key(KeyEvent),
    Paste(String),
    SubmitCommand(String),
    StatusMessage(String),
}
