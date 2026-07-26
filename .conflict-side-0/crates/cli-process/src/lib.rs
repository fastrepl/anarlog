use std::pin::Pin;
use std::time::Duration;

use futures_util::Stream;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

pub type EventStream<T, E> = Pin<Box<dyn Stream<Item = Result<T, E>> + Send>>;

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("process missing stdin")]
    MissingStdin,
    #[error("process missing stdout")]
    MissingStdout,
    #[error("failed to write process stdin: {0}")]
    StdinWrite(#[source] std::io::Error),
    #[error("failed to read process stdout: {0}")]
    StdoutRead(#[source] std::io::Error),
    #[error("failed to wait for process: {0}")]
    Wait(#[source] std::io::Error),
    #[error("failed to kill process: {0}")]
    Kill(#[source] std::io::Error),
    #[error("process exited unsuccessfully: {detail}")]
    ProcessFailed { detail: String },
    #[error("process cancelled")]
    Cancelled,
}

pub struct StreamProcess<T, E> {
    pub events: EventStream<T, E>,
    pub shutdown: CancellationToken,
}

pub fn spawn_with_retry(command: &mut tokio::process::Command) -> std::io::Result<Child> {
    const MAX_EXECUTABLE_BUSY_RETRIES: usize = 5;
    const EXECUTABLE_BUSY_RETRY_DELAY: Duration = Duration::from_millis(20);

    let mut attempts = 0;
    loop {
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(error)
                if error.kind() == std::io::ErrorKind::ExecutableFileBusy
                    && attempts < MAX_EXECUTABLE_BUSY_RETRIES =>
            {
                attempts += 1;
                std::thread::sleep(EXECUTABLE_BUSY_RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}

pub async fn run_to_string(
    mut child: Child,
    cancellation_token: Option<CancellationToken>,
) -> Result<String, ProcessError> {
    if cancellation_token
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
    {
        return Err(ProcessError::Cancelled);
    }

    let stdout = child.stdout.take().ok_or(ProcessError::MissingStdout)?;
    let stderr_task = child.stderr.take().map(spawn_stderr_reader);

    let mut stdout_text = String::new();
    let read_stdout = async {
        let mut reader = BufReader::new(stdout);
        reader
            .read_to_string(&mut stdout_text)
            .await
            .map_err(ProcessError::StdoutRead)
    };

    match cancellation_token.as_ref() {
        Some(token) => tokio::select! {
            _ = token.cancelled() => {
                kill_child(&mut child).await?;
                let _ = collect_stderr(stderr_task).await;
                return Err(ProcessError::Cancelled);
            }
            result = read_stdout => {
                result?;
            }
        },
        None => {
            read_stdout.await?;
        }
    }

    let status = match cancellation_token.as_ref() {
        Some(token) => tokio::select! {
            _ = token.cancelled() => {
                kill_child(&mut child).await?;
                let _ = collect_stderr(stderr_task).await;
                return Err(ProcessError::Cancelled);
            }
            status = child.wait() => status.map_err(ProcessError::Wait)?,
        },
        None => child.wait().await.map_err(ProcessError::Wait)?,
    };

    let stderr_output = collect_stderr(stderr_task).await;
    if !status.success() {
        let detail = if let Some(code) = status.code() {
            format!("code {code}: {}", stderr_output.trim())
        } else {
            stderr_output.trim().to_string()
        };
        return Err(ProcessError::ProcessFailed { detail });
    }

    Ok(stdout_text)
}

pub fn spawn_streaming_lines<T, E, F>(
    mut child: Child,
    stdin_input: Option<String>,
    cancellation_token: Option<CancellationToken>,
    mut parse_line: F,
) -> Result<StreamProcess<T, E>, E>
where
    T: Send + 'static,
    E: From<ProcessError> + Send + 'static,
    F: FnMut(String) -> Result<T, E> + Send + 'static,
{
    if cancellation_token
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
    {
        return Err(E::from(ProcessError::Cancelled));
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| E::from(ProcessError::MissingStdout))?;
    let stderr_task = child.stderr.take().map(spawn_stderr_reader);
    let shutdown = CancellationToken::new();
    let task_shutdown = shutdown.clone();

    let (tx, rx) = mpsc::channel(64);

    tokio::spawn(async move {
        let result: Result<(), E> = async {
            if let Some(input) = stdin_input {
                let mut stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| E::from(ProcessError::MissingStdin))?;
                stdin
                    .write_all(input.as_bytes())
                    .await
                    .map_err(ProcessError::StdinWrite)
                    .map_err(E::from)?;
                stdin
                    .shutdown()
                    .await
                    .map_err(ProcessError::StdinWrite)
                    .map_err(E::from)?;
            }

            let mut lines = BufReader::new(stdout).lines();
            loop {
                let next_line = async { lines.next_line().await.map_err(ProcessError::StdoutRead) };
                let line = match cancellation_token.as_ref() {
                    Some(token) => tokio::select! {
                        _ = token.cancelled() => {
                            kill_child(&mut child).await.map_err(E::from)?;
                            let _ = collect_stderr(stderr_task).await;
                            return Err(E::from(ProcessError::Cancelled));
                        }
                        _ = task_shutdown.cancelled() => {
                            kill_child(&mut child).await.map_err(E::from)?;
                            let _ = collect_stderr(stderr_task).await;
                            return Ok(());
                        }
                        line = next_line => line.map_err(E::from)?,
                    },
                    None => tokio::select! {
                        _ = task_shutdown.cancelled() => {
                            kill_child(&mut child).await.map_err(E::from)?;
                            let _ = collect_stderr(stderr_task).await;
                            return Ok(());
                        }
                        line = next_line => line.map_err(E::from)?,
                    },
                };

                let Some(line) = line else {
                    break;
                };

                let event = parse_line(line)?;
                if tx.send(Ok(event)).await.is_err() {
                    kill_child(&mut child).await.map_err(E::from)?;
                    let _ = collect_stderr(stderr_task).await;
                    return Ok(());
                }
            }

            let status = child
                .wait()
                .await
                .map_err(ProcessError::Wait)
                .map_err(E::from)?;
            let stderr_output = collect_stderr(stderr_task).await;
            if !status.success() {
                let detail = if let Some(code) = status.code() {
                    format!("code {code}: {}", stderr_output.trim())
                } else {
                    stderr_output.trim().to_string()
                };
                return Err(E::from(ProcessError::ProcessFailed { detail }));
            }

            Ok(())
        }
        .await;

        if let Err(error) = result {
            let _ = tx.send(Err(error)).await;
        }
    });

    Ok(StreamProcess {
        events: Box::pin(ReceiverStream::new(rx)),
        shutdown,
    })
}

async fn kill_child(child: &mut Child) -> Result<(), ProcessError> {
    if child.try_wait().map_err(ProcessError::Wait)?.is_some() {
        return Ok(());
    }

    match child.kill().await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(ProcessError::Kill(error)),
    }

    child.wait().await.map_err(ProcessError::Wait)?;
    Ok(())
}

fn spawn_stderr_reader(stderr: tokio::process::ChildStderr) -> JoinHandle<String> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = String::new();
        reader.read_to_string(&mut buf).await.ok();
        buf
    })
}

async fn collect_stderr(stderr_task: Option<JoinHandle<String>>) -> String {
    match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    }
}
