#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn amp: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("failed to kill amp process: {0}")]
    Kill(#[source] std::io::Error),
    #[error("amp process missing stdin")]
    MissingStdin,
    #[error("amp process missing stdout")]
    MissingStdout,
    #[error("failed to write prompt to amp stdin: {0}")]
    StdinWrite(#[source] std::io::Error),
    #[error("failed to read amp stdout: {0}")]
    StdoutRead(#[source] std::io::Error),
    #[error("failed to wait for amp process: {0}")]
    Wait(#[source] std::io::Error),
    #[error("failed to parse event JSON: {0}")]
    ParseEvent(#[from] serde_json::Error),
    #[error("failed to create settings file: {0}")]
    SettingsIo(#[source] std::io::Error),
    #[error("amp exec exited unsuccessfully: {detail}")]
    ProcessFailed { detail: String },
    #[error("turn cancelled")]
    Cancelled,
    #[error("turn failed: {0}")]
    TurnFailed(String),
    #[error("mutex poisoned")]
    Poisoned,
}

impl From<hypr_cli_process::ProcessError> for Error {
    fn from(value: hypr_cli_process::ProcessError) -> Self {
        match value {
            hypr_cli_process::ProcessError::MissingStdin => Self::MissingStdin,
            hypr_cli_process::ProcessError::MissingStdout => Self::MissingStdout,
            hypr_cli_process::ProcessError::StdinWrite(error) => Self::StdinWrite(error),
            hypr_cli_process::ProcessError::StdoutRead(error) => Self::StdoutRead(error),
            hypr_cli_process::ProcessError::Wait(error) => Self::Wait(error),
            hypr_cli_process::ProcessError::Kill(error) => Self::Kill(error),
            hypr_cli_process::ProcessError::ProcessFailed { detail } => {
                Self::ProcessFailed { detail }
            }
            hypr_cli_process::ProcessError::Cancelled => Self::Cancelled,
        }
    }
}
