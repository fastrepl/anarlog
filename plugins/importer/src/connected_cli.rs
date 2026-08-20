use crate::types::{
    ConnectedImportAuthorization, ConnectedImportCredentials, ConnectedImportSyncResult,
    ImportTextFile,
};
use anlg_meeting_import::plaud::{parse_login_url, parse_me};
use anlg_meeting_import::plaud_cli;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;
use tokio::process::Child;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

const PROVIDER_ID: &str = "plaud";
const PROVIDER_NAME: &str = "Plaud";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(45);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(150);
const LOGIN_URL_WAIT: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct ConnectedImportCliState {
    pending: Mutex<Option<PendingCli>>,
}

struct PendingCli {
    binary: PathBuf,
    login: Option<PendingLogin>,
    cancellation: CancellationToken,
}

struct PendingLogin {
    child: Child,
    output: Arc<Mutex<String>>,
}

pub fn is_cli_provider(provider_id: &str) -> bool {
    provider_id == PROVIDER_ID
}

pub async fn begin_connection(
    provider_id: &str,
    state: &ConnectedImportCliState,
) -> Result<ConnectedImportAuthorization, String> {
    ensure_plaud(provider_id)?;
    let binary = plaud_cli::resolve_binary()?;
    match plaud_cli::run(&binary, &["me"], COMMAND_TIMEOUT).await {
        Ok(_) => {
            set_pending(
                state,
                PendingCli {
                    binary,
                    login: None,
                    cancellation: CancellationToken::new(),
                },
            )
            .await;
            return Ok(authorization(""));
        }
        Err(error) if plaud_cli::is_auth_error(&error) => {}
        Err(error) => return Err(error),
    }

    let cancellation = CancellationToken::new();
    let mut login = spawn_login(&binary)?;
    let output = login.output.clone();
    let authorization_url = wait_for_login_url(&mut login, output, cancellation.clone()).await?;

    set_pending(
        state,
        PendingCli {
            binary,
            login: Some(login),
            cancellation,
        },
    )
    .await;
    Ok(authorization(&authorization_url))
}

pub async fn cancel_connection(
    provider_id: &str,
    state: &ConnectedImportCliState,
) -> Result<bool, String> {
    ensure_plaud(provider_id)?;
    let Some(pending) = state.pending.lock().await.take() else {
        return Ok(false);
    };
    pending.cancellation.cancel();
    if let Some(mut login) = pending.login {
        let _ = login.child.kill().await;
    }
    Ok(true)
}

pub async fn complete_connection(
    provider_id: &str,
    state: &ConnectedImportCliState,
) -> Result<ConnectedImportCredentials, String> {
    ensure_plaud(provider_id)?;
    let pending = state
        .pending
        .lock()
        .await
        .take()
        .ok_or_else(|| "Start Plaud sign-in again".to_string())?;
    complete_pending(pending).await
}

pub async fn sync(
    provider_id: &str,
    credentials: ConnectedImportCredentials,
    known_meeting_ids: Vec<String>,
) -> Result<ConnectedImportSyncResult, String> {
    ensure_plaud(provider_id)?;
    let binary = plaud_cli::binary_from_token_json(&credentials.token_json)
        .or_else(|_| plaud_cli::resolve_binary())?;
    let account = parse_me(
        &plaud_cli::run(&binary, &["me"], COMMAND_TIMEOUT)
            .await
            .map_err(|error| {
                if plaud_cli::is_auth_error(&error) {
                    "Reconnect Plaud to keep importing".to_string()
                } else {
                    error
                }
            })?,
    );
    let known = known_meeting_ids.into_iter().collect::<HashSet<_>>();
    let (files, warnings) = plaud_cli::import_new_meetings(&binary, &known).await?;
    Ok(ConnectedImportSyncResult {
        files: files
            .into_iter()
            .map(|file| ImportTextFile {
                path: file.path,
                name: file.name,
                content: file.content,
            })
            .collect(),
        credentials: credentials_for(&binary, &account.display_name()),
        warnings,
    })
}

async fn complete_pending(mut pending: PendingCli) -> Result<ConnectedImportCredentials, String> {
    if let Some(mut login) = pending.login.take() {
        let output = login.output.clone();
        let status = tokio::select! {
            status = login.child.wait() => status.map_err(|error| format!("could not finish Plaud sign-in: {error}"))?,
            _ = pending.cancellation.cancelled() => {
                let _ = login.child.kill().await;
                return Err("Plaud sign-in cancelled.".to_string());
            }
            _ = tokio::time::sleep(LOGIN_TIMEOUT) => {
                let _ = login.child.kill().await;
                return Err("Plaud sign-in timed out. Try again.".to_string());
            }
        };
        if !status.success() {
            let output = output.lock().await;
            return Err(login_error(status.code().unwrap_or(1), &output));
        }
    }

    let stdout = plaud_cli::run(&pending.binary, &["me"], COMMAND_TIMEOUT).await?;
    let account = parse_me(&stdout);
    Ok(credentials_for(&pending.binary, &account.display_name()))
}

async fn wait_for_login_url(
    login: &mut PendingLogin,
    output: Arc<Mutex<String>>,
    cancellation: CancellationToken,
) -> Result<String, String> {
    let deadline = Instant::now() + LOGIN_URL_WAIT;
    loop {
        if cancellation.is_cancelled() {
            let _ = login.child.kill().await;
            return Err("Plaud sign-in cancelled.".to_string());
        }
        if let Some(url) = parse_login_url(&output.lock().await) {
            return Ok(url);
        }
        if let Some(status) = login
            .child
            .try_wait()
            .map_err(|error| format!("could not start Plaud sign-in: {error}"))?
        {
            if status.success() {
                return Ok(String::new());
            }
            let output = output.lock().await;
            return Err(login_error(status.code().unwrap_or(1), &output));
        }
        if Instant::now() >= deadline {
            return Ok(String::new());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn spawn_login(binary: &std::path::Path) -> Result<PendingLogin, String> {
    let mut child = plaud_cli::command(binary, &["login"])
        .spawn()
        .map_err(|error| format!("could not start Plaud sign-in: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "could not read Plaud sign-in output".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "could not read Plaud sign-in output".to_string())?;
    let output = Arc::new(Mutex::new(String::new()));
    let captured = output.clone();
    tokio::spawn(async move {
        capture_output(stdout, stderr, captured).await;
    });
    Ok(PendingLogin { child, output })
}

async fn capture_output<Out, Err>(mut stdout: Out, mut stderr: Err, output: Arc<Mutex<String>>)
where
    Out: AsyncReadExt + Unpin,
    Err: AsyncReadExt + Unpin,
{
    let stdout_output = output.clone();
    let stderr_output = output;
    tokio::join!(
        async move {
            let mut buf = [0_u8; 1024];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => stdout_output
                        .lock()
                        .await
                        .push_str(&String::from_utf8_lossy(&buf[..count])),
                }
            }
        },
        async move {
            let mut buf = [0_u8; 1024];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => stderr_output
                        .lock()
                        .await
                        .push_str(&String::from_utf8_lossy(&buf[..count])),
                }
            }
        }
    );
}

fn credentials_for(binary: &std::path::Path, account: &str) -> ConnectedImportCredentials {
    ConnectedImportCredentials {
        provider_id: PROVIDER_ID.to_string(),
        client_id: account.to_string(),
        client_secret: None,
        token_json: serde_json::json!({
            "kind": "cli",
            "binary": binary.to_string_lossy(),
        })
        .to_string(),
        token_received_at: Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0),
        ),
    }
}

async fn set_pending(state: &ConnectedImportCliState, pending: PendingCli) {
    if let Some(previous) = state.pending.lock().await.replace(pending) {
        previous.cancellation.cancel();
        if let Some(mut login) = previous.login {
            let _ = login.child.kill().await;
        }
    }
}

fn authorization(authorization_url: &str) -> ConnectedImportAuthorization {
    ConnectedImportAuthorization {
        provider_id: PROVIDER_ID.to_string(),
        authorization_url: authorization_url.to_string(),
    }
}

fn ensure_plaud(provider_id: &str) -> Result<(), String> {
    if provider_id == PROVIDER_ID {
        Ok(())
    } else {
        Err(format!("{PROVIDER_NAME} is the only CLI import source"))
    }
}

fn login_error(status: i32, output: &str) -> String {
    if status == 2 || output.contains("AUTH_FAILED") {
        "Plaud sign-in expired. Connect again.".to_string()
    } else {
        let detail = output
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("unknown error");
        format!("could not sign in to Plaud: {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_plaud() {
        assert!(is_cli_provider("plaud"));
        assert!(!is_cli_provider("granola"));
    }
}
