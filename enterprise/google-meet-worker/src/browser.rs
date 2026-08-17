// Adapted for Anarlog from Vexa v0.12.18. See ../THIRD_PARTY_NOTICES.md.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tokio::{process::Child, time::Instant};

const DEVTOOLS_ACTIVE_PORT: &str = "DevToolsActivePort";

#[derive(Debug, Clone)]
pub struct ChromiumLaunchConfig {
    pub binary: PathBuf,
    pub user_data_dir: PathBuf,
    pub locale: String,
    pub authenticated: bool,
    pub headless: bool,
    pub disable_sandbox: bool,
    pub startup_timeout: Duration,
}

impl ChromiumLaunchConfig {
    pub fn launch_args(&self) -> Result<Vec<OsString>, ChromiumLaunchError> {
        validate_locale(&self.locale)?;
        let primary_language = self.locale.split('-').next().unwrap_or(&self.locale);
        let accept_language = if primary_language == self.locale {
            self.locale.clone()
        } else {
            format!("{},{}", self.locale, primary_language)
        };

        let mut args = vec![
            "--disable-blink-features=AutomationControlled".into(),
            "--disable-infobars".into(),
            "--disable-gpu".into(),
            "--in-process-gpu".into(),
            "--use-fake-ui-for-media-stream".into(),
            "--use-file-for-fake-video-capture=/dev/null".into(),
            "--autoplay-policy=no-user-gesture-required".into(),
            "--password-store=basic".into(),
            "--remote-debugging-port=0".into(),
            "--remote-debugging-address=127.0.0.1".into(),
            format!("--user-data-dir={}", self.user_data_dir.display()).into(),
            format!("--lang={}", self.locale).into(),
            format!("--accept-lang={accept_language}").into(),
        ];
        if self.disable_sandbox {
            args.push("--no-sandbox".into());
            args.push("--disable-setuid-sandbox".into());
        }
        if !self.authenticated {
            args.push("--incognito".into());
        }
        if self.headless {
            args.push("--headless=new".into());
        }
        args.push("about:blank".into());
        Ok(args)
    }
}

#[derive(Debug)]
pub struct ChromiumProcess {
    child: Child,
    pub devtools_websocket_url: String,
}

impl ChromiumProcess {
    pub async fn launch(config: ChromiumLaunchConfig) -> Result<Self, ChromiumLaunchError> {
        tokio::fs::create_dir_all(&config.user_data_dir)
            .await
            .map_err(ChromiumLaunchError::ProfileDirectory)?;
        let args = config.launch_args()?;
        let mut command = tokio::process::Command::new(&config.binary);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        let child = command.spawn().map_err(ChromiumLaunchError::Spawn)?;
        let active_port = config.user_data_dir.join(DEVTOOLS_ACTIVE_PORT);
        let mut process = Self {
            child,
            devtools_websocket_url: String::new(),
        };
        process.devtools_websocket_url = process
            .wait_for_devtools(&active_port, config.startup_timeout)
            .await?;
        Ok(process)
    }

    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    pub async fn shutdown(mut self) -> Result<(), std::io::Error> {
        if self.child.try_wait()?.is_none() {
            self.child.kill().await?;
        }
        let _ = self.child.wait().await?;
        Ok(())
    }

    async fn wait_for_devtools(
        &mut self,
        path: &Path,
        timeout: Duration,
    ) -> Result<String, ChromiumLaunchError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(ChromiumLaunchError::ProcessStatus)?
            {
                return Err(ChromiumLaunchError::ExitedBeforeReady(status.code()));
            }
            if let Ok(contents) = tokio::fs::read_to_string(path).await {
                return parse_devtools_active_port(&contents);
            }
            if Instant::now() >= deadline {
                return Err(ChromiumLaunchError::StartupTimeout(timeout));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl Drop for ChromiumProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn validate_locale(locale: &str) -> Result<(), ChromiumLaunchError> {
    if locale.is_empty()
        || !locale
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(ChromiumLaunchError::InvalidLocale(locale.into()));
    }
    Ok(())
}

fn parse_devtools_active_port(contents: &str) -> Result<String, ChromiumLaunchError> {
    let mut lines = contents.lines();
    let port = lines
        .next()
        .and_then(|port| port.parse::<u16>().ok())
        .filter(|port| *port > 0)
        .ok_or_else(|| ChromiumLaunchError::InvalidDevToolsFile(contents.into()))?;
    let path = lines
        .next()
        .filter(|path| path.starts_with("/devtools/browser/"))
        .ok_or_else(|| ChromiumLaunchError::InvalidDevToolsFile(contents.into()))?;
    Ok(format!("ws://127.0.0.1:{port}{path}"))
}

#[derive(Debug, thiserror::Error)]
pub enum ChromiumLaunchError {
    #[error("Chromium locale must be a non-empty language tag: {0}")]
    InvalidLocale(String),
    #[error("failed to prepare the Chromium profile directory")]
    ProfileDirectory(#[source] std::io::Error),
    #[error("failed to launch Chromium")]
    Spawn(#[source] std::io::Error),
    #[error("failed to read Chromium process status")]
    ProcessStatus(#[source] std::io::Error),
    #[error("Chromium exited before DevTools was ready (exit code {0:?})")]
    ExitedBeforeReady(Option<i32>),
    #[error("Chromium did not expose DevTools within {0:?}")]
    StartupTimeout(Duration),
    #[error("invalid Chromium DevToolsActivePort contents: {0:?}")]
    InvalidDevToolsFile(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn config(authenticated: bool) -> ChromiumLaunchConfig {
        ChromiumLaunchConfig {
            binary: "/usr/bin/chromium".into(),
            user_data_dir: "/tmp/anarlog-meet".into(),
            locale: "en-US".into(),
            authenticated,
            headless: false,
            disable_sandbox: false,
            startup_timeout: Duration::from_secs(10),
        }
    }

    #[test]
    fn guest_launch_is_isolated_and_locale_pinned() {
        let args = config(false).launch_args().unwrap();

        assert!(args.contains(&OsString::from("--incognito")));
        assert!(args.contains(&OsString::from("--lang=en-US")));
        assert!(args.contains(&OsString::from("--accept-lang=en-US,en")));
        assert!(args.contains(&OsString::from("--remote-debugging-port=0")));
        assert!(!args.contains(&OsString::from("--no-sandbox")));
        assert!(!args.iter().any(|arg| arg == "--disable-web-security"));
        assert!(!args.iter().any(|arg| arg == "--ignore-certificate-errors"));
    }

    #[test]
    fn sandbox_can_be_disabled_explicitly_for_containerized_chromium() {
        let args = ChromiumLaunchConfig {
            disable_sandbox: true,
            ..config(false)
        }
        .launch_args()
        .unwrap();

        assert!(args.contains(&OsString::from("--no-sandbox")));
        assert!(args.contains(&OsString::from("--disable-setuid-sandbox")));
    }

    #[test]
    fn authenticated_launch_preserves_the_profile() {
        let args = config(true).launch_args().unwrap();

        assert!(!args.contains(&OsString::from("--incognito")));
        assert!(args.contains(&OsString::from("--user-data-dir=/tmp/anarlog-meet")));
    }

    #[test]
    fn parses_chromium_devtools_endpoint() {
        assert_eq!(
            parse_devtools_active_port("49231\n/devtools/browser/test-id\n").unwrap(),
            "ws://127.0.0.1:49231/devtools/browser/test-id"
        );
    }

    #[test]
    fn rejects_shell_like_locale_values() {
        assert!(matches!(
            ChromiumLaunchConfig {
                locale: "en-US;touch-pwned".into(),
                ..config(false)
            }
            .launch_args(),
            Err(ChromiumLaunchError::InvalidLocale(_))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn supervises_and_stops_the_browser_process() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("fake-chromium");
        std::fs::write(
            &executable,
            r#"#!/bin/sh
profile=""
for arg in "$@"; do
  case "$arg" in
    --user-data-dir=*) profile="${arg#--user-data-dir=}" ;;
  esac
done
printf '49231\n/devtools/browser/test-id\n' > "$profile/DevToolsActivePort"
trap 'exit 0' TERM INT
while true; do sleep 1; done
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();

        let process = ChromiumProcess::launch(ChromiumLaunchConfig {
            binary: executable,
            user_data_dir: directory.path().join("profile"),
            locale: "en-US".into(),
            authenticated: false,
            headless: true,
            disable_sandbox: false,
            startup_timeout: Duration::from_secs(2),
        })
        .await
        .unwrap();

        assert!(process.id().is_some());
        assert_eq!(
            process.devtools_websocket_url,
            "ws://127.0.0.1:49231/devtools/browser/test-id"
        );
        process.shutdown().await.unwrap();
    }
}
