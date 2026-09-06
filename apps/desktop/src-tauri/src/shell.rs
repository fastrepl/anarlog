//! Incremental Tauri → GPUI migration: the Tauri binary stays the installed
//! launcher, and when the user has opted into the native shell it execs
//! `anarlog-gpui` (shipped as a sidecar) before creating any webview.

use anlg_storage::shell::{self, Shell};
use std::path::PathBuf;

fn base_dir(identifier: &str) -> Option<PathBuf> {
    anlg_storage::global::compute_default_base(identifier)
}

pub fn preferred(identifier: &str) -> Shell {
    base_dir(identifier)
        .map(|base| shell::effective(&base))
        .unwrap_or(Shell::Tauri)
}

pub fn set_preferred(identifier: &str, target: Shell) -> Result<(), String> {
    let base = base_dir(identifier).ok_or("application data directory is unavailable")?;
    shell::write_preference(&base, target).map_err(|error| error.to_string())
}

pub fn gpui_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    shell::sibling_binary(&exe, shell::GPUI_BINARY)
}

/// Launches GPUI and exits when the user prefers it and the sidecar exists.
/// Falls through (and clears nothing) otherwise, so a missing binary or a
/// crashing GPUI build degrades to the classic app instead of a dead launcher.
pub fn hand_off_if_preferred(identifier: &str) {
    if preferred(identifier) != Shell::Gpui {
        return;
    }
    let Some(binary) = gpui_binary() else {
        tracing::warn!(
            "desktop shell preference is gpui but {} is not installed next to this binary",
            shell::GPUI_BINARY
        );
        return;
    };
    match std::process::Command::new(&binary)
        .arg("--identifier")
        .arg(identifier)
        .spawn()
    {
        Ok(_) => std::process::exit(0),
        Err(error) => {
            tracing::warn!(%error, path = %binary.display(), "failed to launch gpui shell")
        }
    }
}
