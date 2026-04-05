use std::fs;

use crate::error::{CliError, CliResult};

use super::{plist, service};

pub(crate) fn run() -> CliResult<()> {
    let plist_path = plist::plist_path();

    if !plist_path.exists() {
        eprintln!("Shortcut daemon is not installed.");
        return Ok(());
    }

    if let Err(error) = service::stop(&plist_path) {
        eprintln!("Warning: {error}");
    }

    fs::remove_file(&plist_path)
        .map_err(|e| CliError::operation_failed("remove plist", e.to_string()))?;

    eprintln!("Shortcut daemon uninstalled.");
    Ok(())
}
