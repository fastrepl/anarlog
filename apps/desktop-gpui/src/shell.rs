//! Hand-back half of the incremental migration: the Tauri launcher execs this
//! binary when the marker says `gpui`; this module rewrites the marker to
//! `tauri` and relaunches the classic app.

use anlg_storage::shell::{self, Shell};
use std::path::PathBuf;

/// `mainBinaryName` of the Tauri build that owns this identifier.
fn tauri_binary_name(identifier: &str) -> &'static str {
    match identifier {
        "com.hyprnote.dev" => "anarlog-dev",
        "com.hyprnote.staging" => "anarlog-staging",
        _ => "anarlog",
    }
}

pub fn tauri_binary(identifier: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    shell::sibling_binary(&exe, tauri_binary_name(identifier))
}

pub fn switch_to_tauri(identifier: &str) -> anyhow::Result<()> {
    let base = anlg_storage::global::compute_default_base(identifier)
        .ok_or_else(|| anyhow::anyhow!("application data directory is unavailable"))?;
    shell::write_preference(&base, Shell::Tauri)?;
    let binary = tauri_binary(identifier)
        .ok_or_else(|| anyhow::anyhow!("classic app binary is not installed"))?;
    // The launcher re-reads the marker on start, so no flag is needed; the
    // override env var is cleared in case this process was forced into gpui.
    std::process::Command::new(binary)
        .env_remove(shell::ENV_OVERRIDE)
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_identifiers_to_tauri_binaries() {
        assert_eq!(tauri_binary_name("com.hyprnote.dev"), "anarlog-dev");
        assert_eq!(tauri_binary_name("com.hyprnote.staging"), "anarlog-staging");
        assert_eq!(tauri_binary_name("com.hyprnote.stable"), "anarlog");
        assert_eq!(tauri_binary_name("so.anarlog.Anarlog"), "anarlog");
    }
}
