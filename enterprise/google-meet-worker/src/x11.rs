// Adapted for Anarlog from Vexa v0.12.18. See ../THIRD_PARTY_NOTICES.md.

use std::{path::PathBuf, process::Stdio, time::Duration};

#[derive(Debug, Clone)]
pub struct X11InputConfig {
    pub binary: PathBuf,
    pub display: String,
    pub command_timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerLocation {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone)]
pub struct X11Input {
    config: X11InputConfig,
}

impl X11Input {
    pub fn new(config: X11InputConfig) -> Result<Self, X11InputError> {
        if config.display.is_empty() || config.display.contains('\0') {
            return Err(X11InputError::InvalidDisplay);
        }
        if config.command_timeout.is_zero() {
            return Err(X11InputError::InvalidTimeout);
        }
        Ok(Self { config })
    }

    pub async fn verify_available(&self) -> Result<(), X11InputError> {
        self.run(["getdisplaygeometry"]).await.map(|_| ())
    }

    pub async fn pointer_location(&self) -> Result<PointerLocation, X11InputError> {
        let output = self.run(["getmouselocation", "--shell"]).await?;
        let x = parse_shell_coordinate(&output, "X=")?;
        let y = parse_shell_coordinate(&output, "Y=")?;
        Ok(PointerLocation { x, y })
    }

    pub async fn move_absolute(&self, x: i32, y: i32) -> Result<(), X11InputError> {
        self.run(["mousemove".to_owned(), x.to_string(), y.to_string()])
            .await
            .map(|_| ())
    }

    pub async fn button_down(&self) -> Result<(), X11InputError> {
        self.run(["mousedown", "1"]).await.map(|_| ())
    }

    pub async fn button_up(&self) -> Result<(), X11InputError> {
        self.run(["mouseup", "1"]).await.map(|_| ())
    }

    pub async fn type_text(&self, text: &str, delay: Duration) -> Result<(), X11InputError> {
        Self::validate_text(text, delay)?;
        let delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
        self.run([
            "type".to_owned(),
            "--clearmodifiers".to_owned(),
            "--delay".to_owned(),
            delay_ms.to_string(),
            "--".to_owned(),
            text.to_owned(),
        ])
        .await
        .map(|_| ())
    }

    pub fn validate_text(text: &str, delay: Duration) -> Result<(), X11InputError> {
        if text.is_empty() || text.chars().count() > 80 || text.chars().any(char::is_control) {
            return Err(X11InputError::InvalidText);
        }
        let delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
        if !(1..=1_000).contains(&delay_ms) {
            return Err(X11InputError::InvalidTypingDelay(delay));
        }
        Ok(())
    }

    async fn run<I, S>(&self, args: I) -> Result<String, X11InputError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut command = tokio::process::Command::new(&self.config.binary);
        command
            .args(args)
            .env("DISPLAY", &self.config.display)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let output = tokio::time::timeout(self.config.command_timeout, command.output())
            .await
            .map_err(|_| X11InputError::Timeout(self.config.command_timeout))?
            .map_err(X11InputError::Spawn)?;
        if !output.status.success() {
            return Err(X11InputError::CommandFailed {
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        String::from_utf8(output.stdout).map_err(X11InputError::OutputEncoding)
    }
}

fn parse_shell_coordinate(output: &str, prefix: &'static str) -> Result<i32, X11InputError> {
    output
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| X11InputError::PointerOutput(output.into()))
}

#[derive(Debug, thiserror::Error)]
pub enum X11InputError {
    #[error("X11 display must be a non-empty string without NUL bytes")]
    InvalidDisplay,
    #[error("X11 command timeout must be greater than zero")]
    InvalidTimeout,
    #[error("bot display name must contain 1-80 non-control characters")]
    InvalidText,
    #[error("X11 typing delay must be between 1ms and 1s, got {0:?}")]
    InvalidTypingDelay(Duration),
    #[error("failed to execute xdotool")]
    Spawn(#[source] std::io::Error),
    #[error("xdotool timed out after {0:?}")]
    Timeout(Duration),
    #[error("xdotool failed with exit code {code:?}: {stderr}")]
    CommandFailed { code: Option<i32>, stderr: String },
    #[error("xdotool returned non-UTF-8 output")]
    OutputEncoding(#[source] std::string::FromUtf8Error),
    #[error("invalid xdotool pointer output: {0:?}")]
    PointerOutput(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn validates_names_before_passing_them_to_xdotool() {
        assert!(matches!(
            X11Input::validate_text("bad\nname", Duration::from_millis(60)),
            Err(X11InputError::InvalidText)
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn uses_argument_safe_xtest_commands_and_parses_the_pointer() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("xdotool");
        let log = directory.path().join("calls");
        std::fs::write(
            &executable,
            format!(
                r#"#!/bin/sh
printf '%s|%s\n' "$DISPLAY" "$*" >> "{}"
if [ "$1" = "getmouselocation" ]; then
  printf 'X=321\nY=654\nSCREEN=0\nWINDOW=1\n'
fi
"#,
                log.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();

        let input = X11Input::new(X11InputConfig {
            binary: executable,
            display: ":77".into(),
            command_timeout: Duration::from_secs(5),
        })
        .unwrap();
        input.verify_available().await.unwrap();
        input.move_absolute(100, 200).await.unwrap();
        input
            .type_text("A; $(safe)", Duration::from_millis(60))
            .await
            .unwrap();
        assert_eq!(
            input.pointer_location().await.unwrap(),
            PointerLocation { x: 321, y: 654 }
        );

        let calls = std::fs::read_to_string(log).unwrap();
        assert!(calls.contains(":77|mousemove 100 200"));
        assert!(calls.contains(":77|type --clearmodifiers --delay 60 -- A; $(safe)"));
        assert!(calls.contains(":77|getmouselocation --shell"));
    }
}
