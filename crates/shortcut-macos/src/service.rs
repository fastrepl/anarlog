use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ShortcutServiceConfig {
    pub label: String,
    pub program_args: Vec<String>,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
}

impl ShortcutServiceConfig {
    pub fn plist_contents(&self) -> String {
        let args = self
            .program_args
            .iter()
            .map(|arg| format!("        <string>{arg}</string>"))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
{args}
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>StandardOutPath</key>
    <string>{stdout}</string>
    <key>StandardErrorPath</key>
    <string>{stderr}</string>
</dict>
</plist>
"#,
            label = self.label,
            args = args,
            stdout = self.stdout_path.display(),
            stderr = self.stderr_path.display()
        )
    }
}

#[derive(Debug, Clone)]
pub struct ShortcutServiceStatus {
    pub installed: bool,
    pub enabled: bool,
    pub bootstrapped: bool,
    pub running: bool,
    pub pid: Option<u32>,
    pub last_exit_code: Option<i32>,
}

impl ShortcutServiceStatus {
    pub fn reason(&self) -> Option<String> {
        if !self.installed && !self.bootstrapped {
            return None;
        }
        if !self.enabled {
            return Some("LaunchAgent is disabled.".to_string());
        }
        if !self.bootstrapped {
            return Some("LaunchAgent is installed but not bootstrapped.".to_string());
        }
        if let Some(code) = self.last_exit_code {
            if code != 0 {
                return Some(format!(
                    "Daemon exited during startup (last exit code: {code})."
                ));
            }
        }
        if !self.running {
            return Some("LaunchAgent is bootstrapped but not running.".to_string());
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct ShortcutServiceError {
    message: String,
}

impl ShortcutServiceError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ShortcutServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for ShortcutServiceError {}

pub fn query_service(label: &str, plist_path: &Path) -> ShortcutServiceStatus {
    #[cfg(target_os = "macos")]
    {
        let installed = plist_path.exists();
        let enabled = query_enabled(label).unwrap_or(true);
        let print = Command::new("launchctl")
            .args(["print", &service_target(label)])
            .output();

        let mut status = ShortcutServiceStatus {
            installed,
            enabled,
            bootstrapped: false,
            running: false,
            pid: None,
            last_exit_code: None,
        };

        let Ok(output) = print else {
            return status;
        };
        if !output.status.success() {
            return status;
        }

        status.bootstrapped = true;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().map(str::trim) {
            if let Some(value) = line.strip_prefix("pid = ") {
                status.pid = value.parse().ok();
                status.running = status.pid.is_some();
                continue;
            }
            if let Some(value) = line.strip_prefix("state = ") {
                status.running = value == "running" || status.running;
                continue;
            }
            if let Some(value) = line.strip_prefix("last exit code = ") {
                status.last_exit_code = value.parse().ok();
            }
        }

        status
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = label;
        ShortcutServiceStatus {
            installed: plist_path.exists(),
            enabled: false,
            bootstrapped: false,
            running: false,
            pid: None,
            last_exit_code: None,
        }
    }
}

pub fn start_service(label: &str, plist_path: &Path) -> Result<(), ShortcutServiceError> {
    #[cfg(target_os = "macos")]
    {
        run_launchctl(&["enable", &service_target(label)])?;

        let status = query_service(label, plist_path);
        if status.bootstrapped {
            run_launchctl(&["kickstart", "-k", &service_target(label)])?;
        } else {
            run_launchctl(&["bootstrap", &domain_target(), &plist_path.to_string_lossy()])?;
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (label, plist_path);
        Err(ShortcutServiceError::new(
            "Global shortcuts are only supported on macOS.",
        ))
    }
}

pub fn stop_service(label: &str, plist_path: &Path) -> Result<(), ShortcutServiceError> {
    #[cfg(target_os = "macos")]
    {
        let status = query_service(label, plist_path);
        if status.bootstrapped {
            run_launchctl(&["bootout", &domain_target(), &plist_path.to_string_lossy()])?;
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (label, plist_path);
        Err(ShortcutServiceError::new(
            "Global shortcuts are only supported on macOS.",
        ))
    }
}

#[cfg(target_os = "macos")]
fn query_enabled(label: &str) -> Option<bool> {
    let output = Command::new("launchctl")
        .args(["print-disabled", &domain_target()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let needle = format!("\"{label}\" => ");
    for line in String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
    {
        if let Some(value) = line.strip_prefix(&needle) {
            return Some(value == "enabled");
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn run_launchctl(args: &[&str]) -> Result<(), ShortcutServiceError> {
    let output = Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|e| ShortcutServiceError::new(e.to_string()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = if stderr.is_empty() {
        format!("launchctl {} failed", args.join(" "))
    } else {
        stderr
    };
    Err(ShortcutServiceError::new(detail))
}

#[cfg(target_os = "macos")]
fn domain_target() -> String {
    format!("gui/{}", unsafe { libc::getuid() })
}

#[cfg(target_os = "macos")]
fn service_target(label: &str) -> String {
    format!("{}/{}", domain_target(), label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_contents_encode_interactive_launch_agent() {
        let config = ShortcutServiceConfig {
            label: "com.char.shortcut".to_string(),
            program_args: vec!["/tmp/char".to_string(), "shortcut-daemon".to_string()],
            stdout_path: PathBuf::from("/tmp/char.out.log"),
            stderr_path: PathBuf::from("/tmp/char.err.log"),
        };

        let plist = config.plist_contents();
        assert!(plist.contains("<string>Interactive</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("shortcut-daemon"));
    }

    #[test]
    fn reason_prefers_disabled_state() {
        let status = ShortcutServiceStatus {
            installed: true,
            enabled: false,
            bootstrapped: false,
            running: false,
            pid: None,
            last_exit_code: None,
        };

        assert_eq!(status.reason().as_deref(), Some("LaunchAgent is disabled."));
    }

    #[test]
    fn reason_reports_startup_exit_code() {
        let status = ShortcutServiceStatus {
            installed: true,
            enabled: true,
            bootstrapped: true,
            running: false,
            pid: None,
            last_exit_code: Some(1),
        };

        assert_eq!(
            status.reason().as_deref(),
            Some("Daemon exited during startup (last exit code: 1).")
        );
    }
}
