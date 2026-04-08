#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn claude: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("failed to kill claude process: {0}")]
    Kill(#[source] std::io::Error),
    #[error("claude process missing stdout")]
    MissingStdout,
    #[error("failed to read claude stdout: {0}")]
    StdoutRead(#[source] std::io::Error),
    #[error("failed to wait for claude process: {0}")]
    Wait(#[source] std::io::Error),
    #[error("failed to parse claude JSON: {0}")]
    ParseJson(#[from] serde_json::Error),
    #[error("output_schema must be a JSON object")]
    InvalidOutputSchema,
    #[error("claude exec exited unsuccessfully: {detail}")]
    ProcessFailed { detail: String },
    #[error("turn cancelled")]
    Cancelled,
    #[error("turn failed: {0}")]
    TurnFailed(String),
    #[error("mutex poisoned")]
    Poisoned,
}
