use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anlg_desktop_identity::Channel;
use desktop_runtime::{
    Result, ServiceError,
    deeplink::{DeepLink, DeepLinkInbox},
};
use gpui::Application;

#[derive(Clone, Default)]
pub struct NativeDeepLinks {
    inbox: Arc<Mutex<DeepLinkInbox>>,
}

impl NativeDeepLinks {
    pub fn install(&self, application: &Application, notify: impl Fn(Result<u64>) + 'static) {
        let links = self.clone();
        application.on_open_urls(move |urls| {
            for url in urls {
                notify(links.receive(&url));
            }
        });
    }

    pub fn receive(&self, raw: &str) -> Result<u64> {
        let scheme = raw.split_once(':').map(|(scheme, _)| scheme).unwrap_or("");
        if !SCHEMES.contains(&scheme) {
            return Err(failure("Unknown deep-link scheme"));
        }
        self.inbox
            .lock()
            .map_err(|_| ServiceError::Closed)?
            .push(raw)
    }

    pub fn pending(&self, shares: bool) -> Result<Vec<(u64, Arc<DeepLink>)>> {
        Ok(self
            .inbox
            .lock()
            .map_err(|_| ServiceError::Closed)?
            .pending(shares)
            .cloned()
            .collect())
    }

    pub fn acknowledge(&self, id: u64) -> Result<bool> {
        Ok(self
            .inbox
            .lock()
            .map_err(|_| ServiceError::Closed)?
            .acknowledge(id))
    }
}

pub const SCHEMES: &[&str] = &[
    "anarlog",
    "anarlog-staging",
    "anarlog-nightly",
    "anarlog-dev",
    "hyprnote",
    "hyprnote-staging",
    "hyprnote-nightly",
    "hypr",
    "char",
];

/// Shipping's start-up `register_all` on Linux (tauri-plugin-deep-link): a hidden
/// `<executable>-handler.desktop` entry for the channel's schemes, made their default handler.
/// It names the AppImage when the app runs from one, so links follow a moved AppImage. macOS
/// (Info.plist) and Windows (the NSIS installer) register the same schemes at install.
pub fn register_handler(channel: Channel) {
    #[cfg(target_os = "linux")]
    std::thread::spawn(move || {
        if let Err(error) = register_linux_handler(channel) {
            tracing::warn!(%error, "failed to register deep link handlers");
        }
    });
    #[cfg(not(target_os = "linux"))]
    let _ = channel;
}

#[cfg(target_os = "linux")]
fn register_linux_handler(channel: Channel) -> Result<()> {
    use std::{path::PathBuf, process::Command};

    let executable = std::env::current_exe().map_err(failure)?;
    let target = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|| executable.clone());
    let directory = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or_else(|| failure("No data directory"))?
        .join("applications");
    let (name, entry) = handler_entry(channel, &executable, &target)?;
    std::fs::create_dir_all(&directory).map_err(failure)?;
    let path = directory.join(&name);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(entry.as_str()) {
        anlg_storage::fs::atomic_write(&path, &entry).map_err(failure)?;
    }
    // Only refreshes the MIME cache, so a system without it, such as NixOS, still gets the
    // defaults below. Shipping's plugin stops here instead and never sets them.
    if let Err(error) = Command::new("update-desktop-database")
        .arg(&directory)
        .status()
    {
        tracing::debug!(%error, "update-desktop-database is unavailable");
    }
    for scheme in channel.schemes() {
        Command::new("xdg-mime")
            .args(["default", &name, &format!("x-scheme-handler/{scheme}")])
            .status()
            .map_err(failure)?;
    }
    Ok(())
}

/// The handler entry's file name and contents.
pub fn handler_entry(
    channel: Channel,
    executable: &Path,
    target: &Path,
) -> Result<(String, String)> {
    let name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| failure("Executable name is not UTF-8"))?;
    let target = target
        .to_str()
        .ok_or_else(|| failure("Executable path is not UTF-8"))?;
    if target.contains(['\n', '\r', '\0']) {
        return Err(failure("Invalid executable path"));
    }
    let target = target
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('`', "\\\\`")
        .replace('$', "\\\\$")
        .replace('%', "%%");
    let mime_types: String = channel
        .schemes()
        .iter()
        .map(|scheme| format!("x-scheme-handler/{scheme};"))
        .collect();
    Ok((
        format!("{name}-handler.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName={}\nExec=\"{target}\" %u\nTerminal=false\nMimeType={mime_types}\nNoDisplay=true\n",
            channel.product_name()
        ),
    ))
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_every_scheme_a_channel_registers_and_nothing_else() {
        let links = NativeDeepLinks::default();
        for channel in Channel::ALL {
            for scheme in channel.schemes() {
                let link = format!("{scheme}://auth/callback?code=test");
                assert!(links.receive(&link).is_ok(), "{link}");
            }
        }
        assert!(links.receive("https://auth/callback?code=test").is_err());
    }

    #[test]
    fn the_linux_handler_points_the_channels_schemes_at_the_app() {
        let (name, entry) = handler_entry(
            Channel::Staging,
            Path::new("/tmp/.mount_anarlo/usr/bin/anarlog-staging"),
            Path::new("/home/me/Apps/Anarlog \"100%\".AppImage"),
        )
        .unwrap();
        assert_eq!(name, "anarlog-staging-handler.desktop");
        assert_eq!(
            entry,
            "[Desktop Entry]\nType=Application\nName=Anarlog Staging\n\
             Exec=\"/home/me/Apps/Anarlog \\\\\"100%%\\\\\".AppImage\" %u\nTerminal=false\n\
             MimeType=x-scheme-handler/anarlog-staging;x-scheme-handler/hyprnote-staging;\n\
             NoDisplay=true\n"
        );
        assert!(
            handler_entry(
                Channel::Stable,
                Path::new("/usr/bin/anarlog"),
                Path::new("/app\nExec=bad")
            )
            .is_err()
        );
    }
}
