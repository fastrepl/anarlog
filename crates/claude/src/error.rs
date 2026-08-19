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

impl From<anlg_cli_process::ProcessError> for Error {
    fn from(value: anlg_cli_process::ProcessError) -> Self {
        match value {
            anlg_cli_process::ProcessError::MissingStdout => Self::MissingStdout,
            anlg_cli_process::ProcessError::StdoutRead(error) => Self::StdoutRead(error),
            anlg_cli_process::ProcessError::Wait(error) => Self::Wait(error),
            anlg_cli_process::ProcessError::Kill(error) => Self::Kill(error),
            anlg_cli_process::ProcessError::ProcessFailed { detail } => {
                Self::ProcessFailed { detail }
            }
            anlg_cli_process::ProcessError::Cancelled => Self::Cancelled,
            // Claude runs with stdin null, so stdin failures cannot occur.
            anlg_cli_process::ProcessError::MissingStdin => Self::ProcessFailed {
                detail: "process missing stdin".to_string(),
            },
            anlg_cli_process::ProcessError::StdinWrite(error) => Self::ProcessFailed {
                detail: format!("failed to write process stdin: {error}"),
            },
        }
    }
}
