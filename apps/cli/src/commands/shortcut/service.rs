use std::path::Path;

use crate::error::{CliError, CliResult};

pub(crate) use hypr_shortcut_macos::ShortcutServiceStatus as ServiceStatus;

use super::plist;

pub(crate) fn query() -> ServiceStatus {
    hypr_shortcut_macos::query_service(plist::LAUNCHD_LABEL, &plist::plist_path())
}

pub(crate) fn start(plist_path: &Path) -> CliResult<()> {
    hypr_shortcut_macos::start_service(plist::LAUNCHD_LABEL, plist_path)
        .map_err(|e| CliError::operation_failed("launchctl", e.to_string()))
}

pub(crate) fn stop(plist_path: &Path) -> CliResult<()> {
    hypr_shortcut_macos::stop_service(plist::LAUNCHD_LABEL, plist_path)
        .map_err(|e| CliError::operation_failed("launchctl", e.to_string()))
}
