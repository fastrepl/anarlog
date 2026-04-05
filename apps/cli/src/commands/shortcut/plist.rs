use std::path::Path;

use hypr_shortcut_macos::ShortcutServiceConfig;

const LABEL: &str = "com.char.shortcut";

pub(crate) fn service_config(binary_path: &Path) -> ShortcutServiceConfig {
    ShortcutServiceConfig {
        label: LABEL.to_string(),
        program_args: vec![
            binary_path.display().to_string(),
            "shortcut-daemon".to_string(),
        ],
        stdout_path: log_dir().join("shortcut.stdout.log"),
        stderr_path: stderr_log_path(),
    }
}

pub(crate) fn plist_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

pub(crate) fn log_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Logs/char")
}

pub(crate) fn stderr_log_path() -> std::path::PathBuf {
    log_dir().join("shortcut.stderr.log")
}

pub(crate) const LAUNCHD_LABEL: &str = LABEL;
