use crate::json::{ImportFile, meeting_file_with_scheme, meeting_has_content};
use crate::plaud::{
    ListedFile, meeting_from_cli, parse_file_details, parse_files_table, parse_summary,
    parse_transcript, strip_ansi,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;

const PROVIDER_ID: &str = "plaud";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_FILE_PAGES: usize = 20;
const FILE_PAGE_SIZE: u32 = 100;
const MEETING_BATCH_SIZE: usize = 25;

#[derive(Debug, Deserialize)]
struct CliToken {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    binary: Option<String>,
}

pub fn resolve_binary() -> Result<PathBuf, String> {
    find_plaud_in_dirs(search_dirs()).ok_or_else(|| {
        "Install the Plaud CLI (`npm install -g @plaud-ai/cli`) and try again. Anarlog looks for `plaud` on PATH and in common Node.js bin folders.".to_string()
    })
}

pub fn is_allowed_binary(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "plaud" | "plaud.exe" | "plaud.cmd" | "plaud.ps1"))
}

pub fn binary_from_token_json(token_json: &str) -> Result<PathBuf, String> {
    let token = serde_json::from_str::<CliToken>(token_json)
        .map_err(|_| "Reconnect Plaud to keep importing".to_string())?;
    if token.kind.as_deref() != Some("cli") {
        return Err("Reconnect Plaud to keep importing".to_string());
    }
    let Some(binary) = token.binary.filter(|value| !value.is_empty()) else {
        return Err("Reconnect Plaud to keep importing".to_string());
    };
    let path = PathBuf::from(binary);
    if path.is_file() && is_allowed_binary(&path) {
        Ok(path)
    } else {
        resolve_binary()
    }
}

pub fn command(binary: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(binary);
    command
        .args(args)
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1")
        .env("CI", "1")
        .env("PATH", path_for_binary(binary));
    command
}

pub async fn run(binary: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut child = command(binary, args)
        .spawn()
        .map_err(|error| missing_cli_error(&error.to_string()))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let output = Arc::new(Mutex::new(String::new()));
    let capture = if let (Some(stdout), Some(stderr)) = (stdout, stderr) {
        let captured = output.clone();
        Some(tokio::spawn(async move {
            capture_output(stdout, stderr, captured).await;
        }))
    } else {
        None
    };

    let status = tokio::select! {
        status = child.wait() => status.map_err(|error| format!("could not run Plaud CLI: {error}"))?,
        _ = tokio::time::sleep(timeout) => {
            let _ = child.kill().await;
            return Err("Plaud CLI timed out. Try again.".to_string());
        }
    };
    if let Some(capture) = capture {
        let _ = capture.await;
    }
    let captured = output.lock().await.clone();
    if !status.success() {
        return Err(command_error(status.code().unwrap_or(1), &captured));
    }
    Ok(captured)
}

pub async fn import_new_meetings(
    binary: &Path,
    known_meeting_ids: &HashSet<String>,
) -> Result<(Vec<ImportFile>, Vec<String>), String> {
    let listed = list_files(binary).await?;
    let mut files = Vec::new();
    let mut warnings = Vec::new();

    for listed in listed {
        if known_meeting_ids.contains(&listed.id) {
            continue;
        }
        if files.len() >= MEETING_BATCH_SIZE {
            warnings.push(
                "Imported the newest Plaud recordings first. Sync again to bring in older ones."
                    .to_string(),
            );
            break;
        }

        match import_file(binary, &listed.id).await {
            Ok(Some(file)) => files.push(file),
            Ok(None) => {}
            Err(error) => warnings.push(error),
        }
    }

    Ok((files, warnings))
}

async fn list_files(binary: &Path) -> Result<Vec<ListedFile>, String> {
    let mut files = Vec::new();
    for page in 1..=MAX_FILE_PAGES {
        let stdout = run(
            binary,
            &[
                "files",
                "--page",
                &page.to_string(),
                "--page-size",
                &FILE_PAGE_SIZE.to_string(),
            ],
            COMMAND_TIMEOUT,
        )
        .await?;
        let page_files = parse_files_table(&stdout);
        let count = page_files.len();
        files.extend(page_files);
        if count < FILE_PAGE_SIZE as usize {
            break;
        }
    }
    Ok(files)
}

async fn import_file(binary: &Path, id: &str) -> Result<Option<ImportFile>, String> {
    let details = parse_file_details(&run(binary, &["file", id], COMMAND_TIMEOUT).await?);
    if details.id.is_empty() {
        return Err(format!("Plaud recording {id} could not be read"));
    }

    let transcript = match run(binary, &["transcript", id], COMMAND_TIMEOUT).await {
        Ok(stdout) => parse_transcript(&stdout),
        Err(_) => Vec::new(),
    };
    let summary = match run(binary, &["summary", id], COMMAND_TIMEOUT).await {
        Ok(stdout) => parse_summary(&stdout),
        Err(_) => None,
    };

    let meeting = meeting_from_cli(&details, transcript, summary);
    if !meeting_has_content(&meeting) {
        return Ok(None);
    }
    meeting_file_with_scheme("cli", PROVIDER_ID, &meeting).map(Some)
}

async fn capture_output<Out, Err>(mut stdout: Out, mut stderr: Err, output: Arc<Mutex<String>>)
where
    Out: AsyncReadExt + Unpin,
    Err: AsyncReadExt + Unpin,
{
    let stdout_output = output.clone();
    let stderr_output = output;
    let stdout_task = async move {
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
    };
    let stderr_task = async move {
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
    };
    tokio::join!(stdout_task, stderr_task);
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if !cfg!(test) {
        dirs.extend(extra_bin_dirs());
    }
    dirs
}

fn extra_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/opt/homebrew/opt/node/bin"),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".volta/bin"));
        dirs.push(home.join(".asdf/shims"));
        dirs.push(home.join(".npm-global/bin"));
        dirs.push(home.join(".fnm/aliases/default/bin"));
        if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            let mut versions = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path().join("bin"))
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            versions.sort();
            dirs.extend(versions);
        }
    }
    dirs
}

fn path_for_binary(binary: &Path) -> std::ffi::OsString {
    let mut dirs = Vec::new();
    if let Some(parent) = binary.parent() {
        dirs.push(parent.to_path_buf());
    }
    dirs.extend(extra_bin_dirs());
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(dirs).unwrap_or_else(|_| std::env::var_os("PATH").unwrap_or_default())
}

fn find_plaud_in_dirs(dirs: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    let names = if cfg!(windows) {
        ["plaud.exe", "plaud.cmd", "plaud"].as_slice()
    } else {
        ["plaud"].as_slice()
    };
    for dir in dirs {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() && is_allowed_binary(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn command_error(status: i32, output: &str) -> String {
    let output = strip_ansi(output);
    if status == 2 || output.contains("AUTH_FAILED") {
        return "Plaud sign-in expired. Connect again.".to_string();
    }
    let detail = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("unknown error");
    format!("could not run the Plaud CLI: {detail}")
}

fn missing_cli_error(detail: &str) -> String {
    if detail.contains("No such file") || detail.contains("not found") {
        "Install the Plaud CLI (`npm install -g @plaud-ai/cli`) and try again.".to_string()
    } else {
        format!("could not run the Plaud CLI: {detail}")
    }
}

pub fn is_auth_error(error: &str) -> bool {
    error.contains("AUTH_FAILED") || error.contains("sign-in expired")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_arbitrary_binaries() {
        assert!(is_allowed_binary(Path::new("/usr/local/bin/plaud")));
        assert!(!is_allowed_binary(Path::new("/usr/bin/bash")));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn imports_from_official_cli_output() {
        let dir = std::env::temp_dir().join(format!(
            "plaud-cli-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("plaud");
        std::fs::write(
            &binary,
            r#"#!/bin/sh
set -e
cmd="$1"
case "$cmd" in
  me)
    printf '\nUser Info:\n\n  id: user-1\n  email: ada@example.com\n  name: Ada\n'
    ;;
  files)
    printf 'Files on this page: 1\n\n  rec1234567890                    Weekly standup                      2026-08-01    32m10s\n\nPage 1\n'
    ;;
  file)
    printf '\nFile Details:\n\n  id:           rec1234567890\n  name:         Weekly standup\n  created_at:   2026-08-01T10:00:00Z\n  start_at:     -\n  transcript:   available\n  summary:      available\n'
    ;;
  transcript)
    printf '\nTranscript: Weekly standup\n\n[00:01 - 00:04] Ada: Let us ship it.\n'
    ;;
  summary)
    printf '\nSummary: Weekly standup\n\n## Action items\n- Prepare the release\n'
    ;;
  *)
    exit 1
    ;;
esac
"#,
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&binary, permissions).unwrap();

        let (files, warnings) = import_new_meetings(&binary, &HashSet::new()).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "cli://plaud/rec1234567890.json");
        assert!(files[0].content.contains("Weekly standup"));
        assert!(files[0].content.contains("Prepare the release"));
        assert!(warnings.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
