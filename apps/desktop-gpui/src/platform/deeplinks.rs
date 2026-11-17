use std::{
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
};

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
];

pub fn desktop_entry(executable: &Path, schemes: &[&str]) -> Result<String> {
    validate_schemes(schemes)?;
    let executable = executable
        .to_str()
        .ok_or_else(|| failure("Executable path is not UTF-8"))?;
    if executable.contains(['\n', '\r', '\0']) {
        return Err(failure("Invalid executable path"));
    }
    let executable = executable
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('`', "\\\\`")
        .replace('$', "\\\\$")
        .replace('%', "%%");
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=Anarlog GPUI\nNoDisplay=true\nExec=\"{executable}\" %u\nMimeType={}\n",
        schemes
            .iter()
            .map(|scheme| format!("x-scheme-handler/{scheme};"))
            .collect::<String>()
    ))
}

fn validate_schemes(schemes: &[&str]) -> Result<()> {
    if schemes.is_empty() || schemes.iter().any(|scheme| !SCHEMES.contains(scheme)) {
        return Err(failure("Unknown deep-link scheme"));
    }
    Ok(())
}

/// Explicit opt-in registration; packaging chooses only this distribution's schemes.
pub fn register(
    executable: &Path,
    bundle: &Path,
    data_home: &Path,
    schemes: &[&str],
) -> Result<()> {
    validate_schemes(schemes)?;
    #[cfg(target_os = "linux")]
    {
        let _ = bundle;
        let directory = data_home.join("applications");
        std::fs::create_dir_all(&directory).map_err(failure)?;
        anlg_storage::fs::atomic_write(
            &directory.join("anarlog-gpui.desktop"),
            &desktop_entry(executable, schemes)?,
        )
        .map_err(failure)?;
        for scheme in schemes {
            success(Command::new("xdg-mime").args([
                "default",
                "anarlog-gpui.desktop",
                &format!("x-scheme-handler/{scheme}"),
            ]))?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (executable, data_home);
        let output = Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleURLTypes", "json", "-o", "-"])
            .arg(bundle.join("Contents/Info.plist"))
            .output()
            .map_err(failure)?;
        if !output.status.success() {
            return Err(failure("Bundle lacks CFBundleURLTypes"));
        }
        let entries: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(failure)?;
        for scheme in schemes {
            if !entries.as_array().is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry["CFBundleURLSchemes"]
                        .as_array()
                        .is_some_and(|values| {
                            values.iter().any(|value| value.as_str() == Some(scheme))
                        })
                })
            }) {
                return Err(failure("Bundle is missing a requested deep-link scheme"));
            }
        }
        success(Command::new("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister").arg("-f").arg(bundle))?;
    }
    #[cfg(target_os = "windows")]
    {
        let _ = (bundle, data_home);
        let executable = executable
            .to_str()
            .ok_or_else(|| failure("Executable path is not UTF-8"))?;
        if executable.contains(['"', '\r', '\n']) {
            return Err(failure("Invalid executable path"));
        }
        for scheme in schemes {
            let key = format!("HKCU\\Software\\Classes\\{scheme}");
            success(Command::new("reg.exe").args([
                "add",
                &key,
                "/ve",
                "/d",
                "URL:Anarlog GPUI",
                "/f",
            ]))?;
            success(Command::new("reg.exe").args([
                "add",
                &key,
                "/v",
                "URL Protocol",
                "/d",
                "",
                "/f",
            ]))?;
            success(Command::new("reg.exe").args([
                "add",
                &format!("{key}\\shell\\open\\command"),
                "/ve",
                "/d",
                &format!("\"{executable}\" \"%1\""),
                "/f",
            ]))?;
        }
    }
    Ok(())
}

fn success(command: &mut Command) -> Result<()> {
    if command.status().map_err(failure)?.success() {
        Ok(())
    } else {
        Err(failure("Native registration command failed"))
    }
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unregistered_schemes_and_escapes_desktop_commands() {
        assert!(
            NativeDeepLinks::default()
                .receive("https://auth/callback?code=test")
                .is_err()
        );
        assert!(desktop_entry(Path::new("/app\nExec=bad"), &["anarlog-dev"]).is_err());
        assert!(desktop_entry(Path::new("/app"), &["https"]).is_err());
        let entry = desktop_entry(
            Path::new("/Applications/Anarlog 100%/app"),
            &["anarlog-dev"],
        )
        .unwrap();
        assert!(entry.contains("100%%"));
        assert!(entry.contains("x-scheme-handler/anarlog-dev;"));
    }
}
