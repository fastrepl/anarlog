use crossterm::event::KeyEvent;

#[derive(Debug)]
pub(crate) enum Action {
    Key(KeyEvent),
    Paste(String),
    StreamChunk(String),
    StreamCompleted(Option<String>),
    StreamFailed(String),
}
