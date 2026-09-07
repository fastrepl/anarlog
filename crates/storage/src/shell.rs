//! Which desktop shell owns the user's session during the Tauri → GPUI
//! migration. Both binaries share one data directory and one database; a
//! plain marker file next to them records the preferred shell so the Tauri
//! launcher can hand off to GPUI before it creates a webview, and GPUI can
//! hand back.

use std::path::{Path, PathBuf};

pub const MARKER_FILENAME: &str = "desktop-shell";
pub const ENV_OVERRIDE: &str = "ANARLOG_SHELL";
pub const GPUI_BINARY: &str = "anarlog-gpui";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Tauri,
    Gpui,
}

impl Shell {
    pub fn as_str(self) -> &'static str {
        match self {
            Shell::Tauri => "tauri",
            Shell::Gpui => "gpui",
        }
    }

    pub fn parse(value: &str) -> Option<Shell> {
        match value.trim() {
            "tauri" => Some(Shell::Tauri),
            "gpui" => Some(Shell::Gpui),
            _ => None,
        }
    }
}

pub fn marker_path(base: &Path) -> PathBuf {
    base.join(MARKER_FILENAME)
}

/// The persisted preference; Tauri until the user opts in.
pub fn read_preference(base: &Path) -> Shell {
    std::fs::read_to_string(marker_path(base))
        .ok()
        .and_then(|value| Shell::parse(&value))
        .unwrap_or(Shell::Tauri)
}

pub fn write_preference(base: &Path, shell: Shell) -> std::io::Result<()> {
    std::fs::create_dir_all(base)?;
    std::fs::write(marker_path(base), shell.as_str())
}

/// `ANARLOG_SHELL=tauri|gpui` wins over the marker, so a broken opt-in can
/// always be bypassed from a terminal.
pub fn env_override() -> Option<Shell> {
    std::env::var(ENV_OVERRIDE)
        .ok()
        .and_then(|value| Shell::parse(&value))
}

pub fn effective(base: &Path) -> Shell {
    env_override().unwrap_or_else(|| read_preference(base))
}

/// Locates a sibling binary of the running executable: `target/<profile>/` in
/// development and the bundle's binary directory once packaged (Tauri
/// sidecars sit next to the main binary on every platform).
pub fn sibling_binary(current_exe: &Path, name: &str) -> Option<PathBuf> {
    let dir = current_exe.parent()?;
    let candidate = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    candidate.is_file().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_tauri_without_marker() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_preference(dir.path()), Shell::Tauri);
    }

    #[test]
    fn round_trips_preference() {
        let dir = tempfile::tempdir().unwrap();
        write_preference(dir.path(), Shell::Gpui).unwrap();
        assert_eq!(read_preference(dir.path()), Shell::Gpui);
        write_preference(dir.path(), Shell::Tauri).unwrap();
        assert_eq!(read_preference(dir.path()), Shell::Tauri);
    }

    #[test]
    fn ignores_garbage_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(marker_path(dir.path()), "electron\n").unwrap();
        assert_eq!(read_preference(dir.path()), Shell::Tauri);
        assert_eq!(Shell::parse(" gpui\n"), Some(Shell::Gpui));
    }

    #[test]
    fn sibling_binary_requires_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("anarlog");
        assert_eq!(sibling_binary(&exe, GPUI_BINARY), None);
        let sibling = dir
            .path()
            .join(format!("{GPUI_BINARY}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&sibling, b"").unwrap();
        assert_eq!(sibling_binary(&exe, GPUI_BINARY), Some(sibling));
    }
}
