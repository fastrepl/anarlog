use crossterm::event::KeyEvent;

#[derive(Debug)]
pub(crate) enum Action {
    Key(KeyEvent),
    Paste(String),
    StreamChunk(String),
    ToolCallStarted {
        tool_name: String,
        arguments: String,
    },
    StreamCompleted(Option<String>),
    StreamFailed(String),
    TitleGenerated(String),
}
