use crate::error::{CliError, CliResult};

const DOWNLOAD_URL: &str = "https://char.com/download";
const DESKTOP_DEEPLINKS: &[&str] = &[
    "hyprnote://focus",
    "hyprnote-nightly://focus",
    "hyprnote-staging://focus",
    "hypr://focus",
];
const DESKTOP_DATA_FOLDERS: &[&str] = &[
    "hyprnote",
    "com.hyprnote.dev",
    "com.hyprnote.staging",
    "com.hyprnote.nightly",
    "com.hyprnote.stable",
    "com.hyprnote.Hyprnote",
];

pub enum DesktopAction {
    OpenedApp,
    OpenedDownloadPage,
}

pub fn run() -> CliResult<DesktopAction> {
    if !desktop_app_exists() {
        if let Err(e) = open::that(DOWNLOAD_URL) {
            return Err(CliError::operation_failed(
                "open desktop app or download page",
                format!("{e}\nPlease visit: {DOWNLOAD_URL}"),
            ));
        }

        return Ok(DesktopAction::OpenedDownloadPage);
    }

    for deeplink in DESKTOP_DEEPLINKS {
        if open::that(deeplink).is_ok() {
            return Ok(DesktopAction::OpenedApp);
        }
    }

    Err(CliError::operation_failed(
        "open desktop app",
        "Desktop app appears to be installed, but no registered deeplink responded".to_string(),
    ))
}

fn desktop_app_exists() -> bool {
    let Some(data_dir) = dirs::data_dir() else {
        return false;
    };

    DESKTOP_DATA_FOLDERS
        .iter()
        .any(|folder| data_dir.join(folder).exists())
}
