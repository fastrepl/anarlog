//! `tauri-plugin-opener`: `openUrl` and `openPath` hand the target to the
//! system through the `open` crate (`xdg-open` on Linux), `revealItemInDir`
//! asks the file manager to show the item (`org.freedesktop.FileManager1`)
//! and falls back to opening its folder. gpui's own `open_url` /
//! `reveal_path` go through the XDG portal over zbus on its executor, where
//! zbus panics without a tokio context, so on Linux the shell does not use
//! them.

use std::path::{Path, PathBuf};

/// `openerCommands.openUrl(url, null)`.
pub fn open_url(url: &str) {
    if let Err(error) = open::that_detached(url) {
        tracing::warn!(%error, url, "failed to open url");
    }
}

/// `openerCommands.openPath(path, null)`: the file or folder in its default
/// application (a folder in the file manager).
pub fn open_path(path: &Path) {
    if let Err(error) = open::that_detached(path) {
        tracing::warn!(%error, path = %path.display(), "failed to open path");
    }
}

/// `openerCommands.revealItemInDir(path)`: the item selected in its folder.
pub fn reveal_item_in_dir(runtime: &tokio::runtime::Handle, path: PathBuf) {
    let path = std::fs::canonicalize(&path).unwrap_or(path);
    runtime.spawn(async move {
        if let Err(error) = platform::reveal(&path).await {
            tracing::debug!(%error, path = %path.display(), "file manager did not show the item");
            let fallback = if path.is_dir() {
                path.clone()
            } else {
                path.parent().map(Path::to_path_buf).unwrap_or(path)
            };
            open_path(&fallback);
        }
    });
}

#[cfg(target_os = "linux")]
mod platform {
    use std::path::Path;

    /// `reveal_with_filemanager1`: `ShowItems([uri], "")` on the session bus.
    pub async fn reveal(path: &Path) -> anyhow::Result<()> {
        let uri = url::Url::from_file_path(path)
            .map_err(|_| anyhow::anyhow!("path is not a file url: {}", path.display()))?;
        let connection = zbus::Connection::session().await?;
        connection
            .call_method(
                Some("org.freedesktop.FileManager1"),
                "/org/freedesktop/FileManager1",
                Some("org.freedesktop.FileManager1"),
                "ShowItems",
                &(vec![uri.as_str()], ""),
            )
            .await?;
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use std::path::Path;

    /// The plugin uses the platform's own reveal (Finder / Explorer) there;
    /// opening the folder is the nearest the shell has without it.
    pub async fn reveal(_path: &Path) -> anyhow::Result<()> {
        anyhow::bail!("no file manager reveal on this platform")
    }
}
