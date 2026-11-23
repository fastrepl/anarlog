//! Self-update like shipping's `tauri-plugin-updater`: the channel's feed and signing key,
//! and per package the artifact it publishes and the way it installs it.

mod deb;
mod feed;

use std::{
    fs,
    io::{Cursor, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};

use desktop_runtime::{
    Result, ServiceError,
    updater::{UpdateConfig, VerifiedUpdate},
};

use crate::platform::{
    package::{self, BundleType, Os},
    windows::Restart,
};

/// The flags shipping's updater runs the NSIS installer with (`installMode` "passive"): a
/// progress-only install that relaunches the app, followed by the app's own arguments.
pub const NSIS_UPDATE_ARGS: [&str; 4] = ["/P", "/R", "/UPDATE", "/ARGS"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageKind {
    AppImage,
    Deb,
    MacBundle,
    WindowsNsis,
}

impl PackageKind {
    /// The package this binary was installed from, which decides how an update replaces it.
    pub fn current() -> Result<Self> {
        Self::of(package::bundle_type())
    }

    /// Shipping publishes updates for these packages only (`desktop-release-platforms.json`).
    fn of(bundle: Option<BundleType>) -> Result<Self> {
        match bundle {
            Some(BundleType::App) => Ok(Self::MacBundle),
            Some(BundleType::AppImage) => Ok(Self::AppImage),
            Some(BundleType::Deb) => Ok(Self::Deb),
            Some(BundleType::Nsis) => Ok(Self::WindowsNsis),
            Some(BundleType::Rpm | BundleType::Msi) | None => Err(ServiceError::Unsupported(
                "This build can't update itself. Install the new version instead.".into(),
            )),
        }
    }

    fn bundle(self) -> BundleType {
        match self {
            Self::AppImage => BundleType::AppImage,
            Self::Deb => BundleType::Deb,
            Self::MacBundle => BundleType::App,
            Self::WindowsNsis => BundleType::Nsis,
        }
    }
}

/// `tauri-plugin-updater`'s `{{arch}}`.
fn arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86" => "i686",
        "arm" => "armv7",
        arch => arch,
    }
}

/// This build's feed (`feed::feed`): shipping's for its channel and package, in a channel run.
/// Builds without a channel have none.
pub fn config(channel_run: bool) -> Result<UpdateConfig> {
    let none = || ServiceError::Unsupported("This build doesn't update itself.".into());
    let channel = package::channel().ok_or_else(none)?;
    let kind = PackageKind::current()?;
    let feed = feed::feed(
        channel,
        channel_run,
        Os::CURRENT,
        arch(),
        kind.bundle(),
        std::env::var(feed::ENDPOINT_OVERRIDE).ok(),
    )
    .ok_or_else(none)?;
    let endpoints = feed
        .endpoints
        .iter()
        .map(|endpoint| url::Url::parse(endpoint))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| failure("Invalid update feed URL"))?;
    let version = package::VERSION
        .parse()
        .map_err(|_| failure("Invalid app version"))?;
    Ok(UpdateConfig::new(
        endpoints,
        version,
        feed.target,
        feed.pubkey,
    ))
}

/// The installed package an update replaces.
#[derive(Clone, Debug)]
pub struct NativeInstaller {
    kind: PackageKind,
    /// The `.app` bundle or the AppImage file; the deb and NSIS installers find their own.
    installation: PathBuf,
}

impl NativeInstaller {
    pub fn current() -> Result<Self> {
        let kind = PackageKind::current()?;
        let installation = match kind {
            PackageKind::MacBundle => std::env::current_exe()
                .ok()
                .as_deref()
                .and_then(app_bundle)
                .ok_or_else(|| failure("This app isn't running from its app bundle"))?,
            PackageKind::AppImage => std::env::var_os("APPIMAGE")
                .map(PathBuf::from)
                .ok_or_else(|| failure("This app isn't running from its AppImage"))?,
            PackageKind::Deb | PackageKind::WindowsNsis => {
                std::env::current_exe().map_err(failure)?
            }
        };
        Ok(Self { kind, installation })
    }

    /// Installs the verified update and returns what has to run once the app has quit.
    pub async fn install(self, update: VerifiedUpdate) -> Result<Restart> {
        tokio::task::spawn_blocking(move || self.install_verified(update.bytes()))
            .await
            .map_err(failure)?
    }

    fn install_verified(&self, bytes: &[u8]) -> Result<Restart> {
        match self.kind {
            PackageKind::AppImage => {
                if !cfg!(target_os = "linux") {
                    return Err(failure("AppImage installation requires Linux"));
                }
                let parent = self
                    .installation
                    .parent()
                    .ok_or_else(|| failure("Missing installation parent"))?;
                let stage = tempfile::tempdir_in(parent).map_err(failure)?;
                let package = if bytes.starts_with(b"\x7fELF") {
                    let path = stage.path().join("update.AppImage");
                    fs::write(&path, bytes).map_err(failure)?;
                    path
                } else {
                    extract_single_package(bytes, stage.path(), false)?
                };
                fs::set_permissions(
                    &package,
                    fs::metadata(&self.installation)
                        .map_err(failure)?
                        .permissions(),
                )
                .map_err(failure)?;
                fs::File::open(&package)
                    .map_err(failure)?
                    .sync_all()
                    .map_err(failure)?;
                replace_with_backup(&package, &self.installation)?;
            }
            PackageKind::Deb => {
                if !cfg!(target_os = "linux") {
                    return Err(failure("Debian package installation requires Linux"));
                }
                let (_stage, package) = stage_deb_package(bytes)?;
                deb::install(&package)?;
            }
            PackageKind::MacBundle => {
                if !cfg!(target_os = "macos") {
                    return Err(failure("App bundle installation requires macOS"));
                }
                self.install_bundle(bytes)?;
            }
            PackageKind::WindowsNsis => {
                if !cfg!(target_os = "windows") {
                    return Err(failure("Windows installer requires Windows"));
                }
                if !bytes.starts_with(b"MZ") {
                    return Err(failure("Expected a signed NSIS executable"));
                }
                // It runs once the app has quit, so it outlives this process.
                let stage = tempfile::Builder::new()
                    .prefix("anarlog-updater-")
                    .tempdir()
                    .map_err(failure)?
                    .keep();
                let path = stage.join("installer.exe");
                let mut file = fs::File::create(&path).map_err(failure)?;
                file.write_all(bytes).map_err(failure)?;
                file.sync_all().map_err(failure)?;
                return Ok(Restart::Installer(path));
            }
        }
        Ok(Restart::App)
    }

    /// The new bundle must carry the running app's designated requirement (its identifier and
    /// signing team), which its privacy permissions and keychain access are granted to.
    fn install_bundle(&self, bytes: &[u8]) -> Result<()> {
        let requirement = designated_requirement(&self.installation)?;
        let parent = self
            .installation
            .parent()
            .ok_or_else(|| failure("Missing bundle parent"))?;
        match tempfile::tempdir_in(parent) {
            Ok(stage) => {
                let package = extract_single_package(bytes, stage.path(), true)?;
                verify_bundle(&package, &requirement)?;
                replace_with_backup(&package, &self.installation)?;
            }
            // Like shipping's updater, a folder the user can't write asks for an administrator.
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                let stage = tempfile::tempdir().map_err(failure)?;
                let package = extract_single_package(bytes, stage.path(), true)?;
                verify_bundle(&package, &requirement)?;
                replace_as_administrator(&package, &self.installation, stage.path())?;
            }
            Err(error) => return Err(failure(error)),
        }
        // Shipping touches the bundle so Finder and Launch Services see the new version.
        let _ = Command::new("/usr/bin/touch")
            .arg(&self.installation)
            .status();
        Ok(())
    }
}

/// `/Applications/Anarlog.app` for `/Applications/Anarlog.app/Contents/MacOS/anarlog`.
fn app_bundle(executable: &Path) -> Option<PathBuf> {
    let macos = executable.parent()?;
    let bundle = macos.parent()?.parent()?;
    (macos.file_name()? == "MacOS"
        && macos.parent()?.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then(|| bundle.to_owned())
}

fn designated_requirement(bundle: &Path) -> Result<String> {
    let output = Command::new("/usr/bin/codesign")
        .args(["-d", "-r-"])
        .arg(bundle)
        .output()
        .map_err(failure)?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("designated => "))
        .map(str::to_owned)
        .ok_or_else(|| failure("The running app has no code signature to match"))
}

fn verify_bundle(bundle: &Path, requirement: &str) -> Result<()> {
    let verified = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(format!("-R={requirement}"))
        .arg(bundle)
        .status()
        .map_err(failure)?
        .success();
    if verified {
        Ok(())
    } else {
        Err(failure(
            "The update isn't signed as this app; it was not installed",
        ))
    }
}

fn stage_deb_package(bytes: &[u8]) -> Result<(tempfile::TempDir, PathBuf)> {
    if !bytes.starts_with(b"!<arch>\ndebian-binary") {
        return Err(failure("Expected a Debian package"));
    }
    let stage = tempfile::Builder::new()
        .prefix("anarlog_deb_update")
        .tempdir()
        .map_err(failure)?;
    let package = stage.path().join("package.deb");
    fs::write(&package, bytes).map_err(failure)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Allow `_apt` to traverse the staging directory and read `package.deb` without
        // falling back to an unsandboxed root download.
        fs::set_permissions(&package, fs::Permissions::from_mode(0o644)).map_err(failure)?;
        fs::set_permissions(stage.path(), fs::Permissions::from_mode(0o755)).map_err(failure)?;
    }
    Ok((stage, package))
}

const ADMIN_REPLACE_SCRIPT: &str = r#"on run argv
    set currentPath to quoted form of (item 1 of argv)
    set previousPath to quoted form of (item 2 of argv)
    set newPath to quoted form of (item 3 of argv)
    do shell script ("mv -f -- " & currentPath & " " & previousPath & " && (mv -f -- " & newPath & " " & currentPath & " || (mv -f -- " & previousPath & " " & currentPath & "; exit 1))") with administrator privileges
end run"#;

fn admin_replace_args(package: &Path, installation: &Path, stage: &Path) -> Result<[String; 3]> {
    let argument = |path: &Path| {
        path.to_str()
            .filter(|text| !text.contains('\0'))
            .map(str::to_owned)
            .ok_or_else(|| failure("Unsupported installation path"))
    };
    Ok([
        argument(installation)?,
        argument(&stage.join("previous.app"))?,
        argument(package)?,
    ])
}

/// Shipping's `do shell script … with administrator privileges` move, keeping the replaced
/// bundle in the stage until the new one is in place.
fn replace_as_administrator(package: &Path, installation: &Path, stage: &Path) -> Result<()> {
    let args = admin_replace_args(package, installation, stage)?;
    let moved = Command::new("/usr/bin/osascript")
        .args(["-e", ADMIN_REPLACE_SCRIPT, "--"])
        .args(&args)
        .status()
        .map_err(failure)?
        .success();
    if moved {
        Ok(())
    } else {
        Err(failure("The new app was not moved into place"))
    }
}

fn extract_single_package(bytes: &[u8], destination: &Path, bundle: bool) -> Result<PathBuf> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(Cursor::new(bytes)));
    let mut total = 0u64;
    for (index, entry) in archive.entries().map_err(failure)?.enumerate() {
        let mut entry = entry.map_err(failure)?;
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| failure("Archive too large"))?;
        if index >= 100_000 || total > 4 * 1024 * 1024 * 1024 {
            return Err(failure("Archive exceeds installation limits"));
        }
        let path = entry.path().map_err(failure)?.into_owned();
        if !relative_path(&path) {
            return Err(failure("Archive contains an unsafe path"));
        }
        let kind = entry.header().entry_type();
        if kind.is_symlink() {
            let link = entry
                .link_name()
                .map_err(failure)?
                .ok_or_else(|| failure("Missing symlink target"))?;
            if !relative_path(&path.parent().unwrap_or(Path::new("")).join(link)) {
                return Err(failure("Symlink leaves the installation"));
            }
        } else if !kind.is_file() && !kind.is_dir() {
            return Err(failure("Archive contains unsupported entry type"));
        }
        if !entry.unpack_in(destination).map_err(failure)? {
            return Err(failure("Archive entry escaped destination"));
        }
    }
    let paths = fs::read_dir(destination)
        .map_err(failure)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(failure)?;
    if paths.len() != 1 {
        return Err(failure("Expected one application package"));
    }
    let path = paths
        .into_iter()
        .next()
        .ok_or_else(|| failure("Empty package"))?;
    if bundle {
        if path.extension().is_none_or(|extension| extension != "app")
            || !path.join("Contents/Info.plist").is_file()
        {
            return Err(failure("Expected an app bundle"));
        }
    } else if !path.is_file()
        || path
            .extension()
            .is_none_or(|extension| extension != "AppImage")
    {
        return Err(failure("Expected one AppImage"));
    }
    Ok(path)
}

fn relative_path(path: &Path) -> bool {
    let mut depth = 0;
    for component in path.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            _ => return false,
        }
    }
    depth > 0
}

/// Keeps the replaced version as the one rollback copy; older copies, whose versions ran
/// well enough to install this update, are removed.
fn replace_with_backup(package: &Path, installation: &Path) -> Result<PathBuf> {
    let backup = installation.with_extension(format!("previous-{}", uuid::Uuid::new_v4()));
    fs::rename(installation, &backup).map_err(failure)?;
    if let Err(error) = fs::rename(package, installation) {
        fs::rename(&backup, installation).map_err(failure)?;
        return Err(failure(error));
    }
    for old in older_backups(installation, &backup) {
        let removed = if old.is_dir() {
            fs::remove_dir_all(&old)
        } else {
            fs::remove_file(&old)
        };
        if let Err(error) = removed {
            tracing::warn!(%error, path = %old.display(), "old update backup was not removed");
        }
    }
    Ok(backup)
}

fn older_backups(installation: &Path, keep: &Path) -> Vec<PathBuf> {
    let (Some(parent), Some(stem)) = (installation.parent(), installation.file_stem()) else {
        return Vec::new();
    };
    let prefix = format!("{}.previous-", stem.to_string_lossy());
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path != keep)
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str()?.strip_prefix(&prefix))
                .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        })
        .collect()
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_packages_shipping_publishes_updates_for_update_themselves() {
        for (bundle, kind) in [
            (BundleType::App, PackageKind::MacBundle),
            (BundleType::AppImage, PackageKind::AppImage),
            (BundleType::Deb, PackageKind::Deb),
            (BundleType::Nsis, PackageKind::WindowsNsis),
        ] {
            assert_eq!(PackageKind::of(Some(bundle)).unwrap(), kind);
        }
        for bundle in [Some(BundleType::Rpm), Some(BundleType::Msi), None] {
            assert!(PackageKind::of(bundle).is_err(), "{bundle:?}");
        }
    }

    #[test]
    fn a_bundle_update_replaces_the_app_the_executable_runs_from() {
        assert_eq!(
            app_bundle(Path::new(
                "/Applications/Anarlog.app/Contents/MacOS/anarlog"
            )),
            Some(PathBuf::from("/Applications/Anarlog.app"))
        );
        for exe in [
            "/usr/local/bin/anarlog",
            "/Applications/Anarlog/Contents/MacOS/anarlog",
            "/Applications/Anarlog.app/Contents/Resources/anarlog",
        ] {
            assert_eq!(app_bundle(Path::new(exe)), None, "{exe}");
        }
    }

    #[test]
    fn replacement_retains_one_rollback_and_rejects_unsafe_archive_paths() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("Anarlog.app");
        fs::create_dir(&old).unwrap();
        fs::write(old.join("version"), b"1").unwrap();
        let unrelated = root.path().join("Anarlog.previous-notes");
        fs::write(&unrelated, b"keep").unwrap();
        let mut backup = PathBuf::new();
        for version in [b"2", b"3"] {
            let new = root.path().join("new");
            fs::create_dir(&new).unwrap();
            fs::write(new.join("version"), version).unwrap();
            backup = replace_with_backup(&new, &old).unwrap();
        }
        assert_eq!(fs::read(old.join("version")).unwrap(), b"3");
        assert_eq!(fs::read(backup.join("version")).unwrap(), b"2");
        let mut left = fs::read_dir(root.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        left.sort();
        let mut expected = vec![old.clone(), backup, unrelated];
        expected.sort();
        assert_eq!(left, expected, "only the latest rollback copy remains");
        assert!(!relative_path(Path::new("../outside")));
        assert!(!relative_path(Path::new("/absolute")));
        assert!(relative_path(Path::new("App.app/Contents/Resources")));
        assert!(replace_with_backup(&root.path().join("missing"), &old).is_err());
        assert_eq!(fs::read(old.join("version")).unwrap(), b"3");
    }

    #[test]
    fn deb_staging_rejects_invalid_headers_and_sets_apt_readable_permissions() {
        assert!(stage_deb_package(b"not a deb").is_err());
        #[cfg_attr(not(unix), allow(unused_variables))]
        let (stage, package) = stage_deb_package(b"!<arch>\ndebian-binary").unwrap();
        assert_eq!(fs::read(&package).unwrap(), b"!<arch>\ndebian-binary");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(stage.path()).unwrap().permissions().mode() & 0o777,
                0o755
            );
            assert_eq!(
                fs::metadata(&package).unwrap().permissions().mode() & 0o777,
                0o644
            );
        }
    }

    #[test]
    fn admin_replace_args_accept_quotes_spaces_and_backslashes_without_nul() {
        let stage = Path::new(r"/tmp/stage'dir");
        let installation = Path::new(r#"/Applications/Anarlog "Dev"\App.app"#);
        let package = Path::new(r"/tmp/stage'dir/Anarlog's Update.app");
        let args = admin_replace_args(package, installation, stage).unwrap();
        assert_eq!(
            args,
            [
                r#"/Applications/Anarlog "Dev"\App.app"#,
                r"/tmp/stage'dir/previous.app",
                r"/tmp/stage'dir/Anarlog's Update.app",
            ]
        );
        assert!(admin_replace_args(Path::new("/tmp/bad\0.app"), installation, stage).is_err());
    }
}
