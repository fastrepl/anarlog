//! Self-update like shipping's `tauri-plugin-updater`: the channel's feed and signing key,
//! and per package the artifact it publishes and the way it installs it.

mod deb;
mod feed;

use std::{
    fs,
    io::{Cursor, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use desktop_runtime::{
    Result, ServiceError,
    updater::{UpdateConfig, VerifiedUpdate},
};
use sha2::Digest as _;

use crate::platform::{
    package::{self, BundleType, Os},
    windows::Restart,
};

/// The flags shipping's updater runs the NSIS installer with (`installMode` "passive"): a
/// progress-only install that relaunches the app, followed by the app's own arguments.
pub const NSIS_UPDATE_ARGS: [&str; 4] = ["/P", "/R", "/UPDATE", "/ARGS"];
const WINDOWS_INSTALLER_PREFIX: &str = "anarlog-updater-";
const STAGE_LOCK_FILE: &str = "stage.lock";

static EXEC_LOCK: Mutex<Option<ExecutableLock>> = Mutex::new(None);
static STAGED_INSTALLER_LOCK: Mutex<Option<fs::File>> = Mutex::new(None);

struct ExecutableLock {
    path: PathBuf,
    file: Option<fs::File>,
}

fn open_lock_file(path: &Path) -> Result<fs::File> {
    fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(failure)
}

impl ExecutableLock {
    fn acquire_shared(path: PathBuf) -> Result<Self> {
        let file = open_lock_file(&path)?;
        file.try_lock_shared().map_err(|error| match error {
            fs::TryLockError::WouldBlock => {
                failure("Another Anarlog update is holding the executable lock")
            }
            fs::TryLockError::Error(error) => failure(error),
        })?;
        Ok(Self {
            path,
            file: Some(file),
        })
    }

    fn try_upgrade_exclusive(&mut self) -> Result<bool> {
        drop(self.file.take());
        let file = open_lock_file(&self.path)?;
        match file.try_lock() {
            Ok(()) => {
                self.file = Some(file);
                Ok(true)
            }
            Err(fs::TryLockError::WouldBlock) => {
                drop(file);
                self.downgrade_to_shared();
                Ok(false)
            }
            Err(fs::TryLockError::Error(error)) => {
                drop(file);
                self.downgrade_to_shared();
                Err(failure(error))
            }
        }
    }

    fn downgrade_to_shared(&mut self) {
        drop(self.file.take());
        if let Ok(file) = open_lock_file(&self.path)
            && file.try_lock_shared().is_ok()
        {
            self.file = Some(file);
        }
    }
}

fn executable_lock_path() -> Result<PathBuf> {
    let exe = std::env::current_exe()
        .and_then(|path| fs::canonicalize(&path).or(Ok(path)))
        .map_err(failure)?;
    let digest = sha2::Sha256::digest(exe.as_os_str().as_encoded_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(std::env::temp_dir().join(format!("anarlog-exec-{hex}.lock")))
}

/// Holds a shared lock for the running executable so Windows self-updates detect concurrent
/// profiles before NSIS passive mode force-terminates processes sharing this binary name.
pub fn hold_executable_lock() {
    let mut guard = EXEC_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    if guard.is_some() {
        return;
    }
    match executable_lock_path().and_then(ExecutableLock::acquire_shared) {
        Ok(lock) => *guard = Some(lock),
        Err(error) => tracing::warn!(%error, "could not acquire executable lock"),
    }
}

/// Releases this process's executable lock right before handing off to a restart or installer.
pub fn release_executable_lock() {
    let mut guard = EXEC_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    guard.take();
}

fn ensure_sole_executable_instance() -> Result<()> {
    let mut guard = EXEC_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    if guard.is_none() {
        let path = executable_lock_path()?;
        *guard = Some(ExecutableLock::acquire_shared(path)?);
    }
    let lock = guard
        .as_mut()
        .ok_or_else(|| failure("Missing executable lock"))?;
    if lock.try_upgrade_exclusive()? {
        Ok(())
    } else {
        Err(failure(
            "Close other Anarlog profiles before installing this update.",
        ))
    }
}

fn downgrade_executable_lock() {
    let mut guard = EXEC_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(lock) = guard.as_mut() {
        lock.downgrade_to_shared();
    }
}

/// Prunes completed NSIS staging directories left by earlier Windows updates.
pub fn cleanup_stale_installers() {
    cleanup_stale_installers_in(&std::env::temp_dir());
}

fn cleanup_stale_installers_in(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(WINDOWS_INSTALLER_PREFIX) {
            continue;
        }
        let stage_lock = path.join(STAGE_LOCK_FILE);
        if stage_lock.exists() {
            let Ok(lock) = open_lock_file(&stage_lock) else {
                continue;
            };
            if lock.try_lock().is_err() {
                continue;
            }
            drop(lock);
        }
        let installer = path.join("installer.exe");
        if installer.exists() && fs::remove_file(&installer).is_err() {
            continue;
        }
        let _ = fs::remove_dir_all(&path);
    }
}

fn stage_windows_installer_in(bytes: &[u8], root: &Path) -> Result<(PathBuf, fs::File)> {
    if !bytes.starts_with(b"MZ") {
        return Err(failure("Expected a signed NSIS executable"));
    }
    // Keep the TempDir guard until write and sync finish so failures clean up automatically, and
    // hold `stage.lock` until this process exits after spawning NSIS so sibling channels don't
    // prune a pending installer mid-drain.
    let stage = tempfile::Builder::new()
        .prefix(WINDOWS_INSTALLER_PREFIX)
        .tempdir_in(root)
        .map_err(failure)?;
    let lock = open_lock_file(&stage.path().join(STAGE_LOCK_FILE))?;
    lock.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => failure("Could not lock staged installer"),
        fs::TryLockError::Error(error) => failure(error),
    })?;
    let path = stage.path().join("installer.exe");
    let mut file = fs::File::create(&path).map_err(failure)?;
    file.write_all(bytes).map_err(failure)?;
    file.sync_all().map_err(failure)?;
    let _ = stage.keep();
    Ok((path, lock))
}

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
                ensure_sole_executable_instance()?;
                cleanup_stale_installers();
                match stage_windows_installer_in(bytes, &std::env::temp_dir()) {
                    Ok((path, lock)) => {
                        *STAGED_INSTALLER_LOCK
                            .lock()
                            .unwrap_or_else(|error| error.into_inner()) = Some(lock);
                        return Ok(Restart::Installer(path));
                    }
                    Err(error) => {
                        downgrade_executable_lock();
                        return Err(error);
                    }
                }
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
                replace_as_administrator(&package, &self.installation)?;
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
    set newPath to quoted form of (item 2 of argv)
    do shell script ("current=" & currentPath & "; new=" & newPath & "; parent=$(dirname -- \"$current\") && stage=$(mktemp -d \"$parent/.anarlog-update.XXXXXX\") || exit 1; trap 'rm -rf -- \"$stage\"' EXIT; mv -f -- \"$new\" \"$stage/incoming.app\" || exit 1; mv -f -- \"$current\" \"$stage/previous.app\" || exit 1; if mv -f -- \"$stage/incoming.app\" \"$current\"; then exit 0; fi; rm -rf -- \"$current\"; if mv -f -- \"$stage/previous.app\" \"$current\"; then exit 1; fi; trap - EXIT; exit 1") with administrator privileges
end run"#;

fn admin_replace_args(package: &Path, installation: &Path) -> Result<[String; 2]> {
    let argument = |path: &Path| {
        path.to_str()
            .filter(|text| !text.contains('\0'))
            .map(str::to_owned)
            .ok_or_else(|| failure("Unsupported installation path"))
    };
    Ok([argument(installation)?, argument(package)?])
}

/// Stages the incoming and previous bundles in an administrator-owned directory beside the
/// install, replaces atomically on that volume, and removes the stage once complete.
fn replace_as_administrator(package: &Path, installation: &Path) -> Result<()> {
    let args = admin_replace_args(package, installation)?;
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
        let installation = Path::new(r#"/Applications/Anarlog "Dev"\App.app"#);
        let package = Path::new(r"/tmp/stage'dir/Anarlog's Update.app");
        let args = admin_replace_args(package, installation).unwrap();
        assert_eq!(
            args,
            [
                r#"/Applications/Anarlog "Dev"\App.app"#,
                r"/tmp/stage'dir/Anarlog's Update.app",
            ]
        );
        assert!(admin_replace_args(Path::new("/tmp/bad\0.app"), installation).is_err());
    }

    #[test]
    fn executable_lock_blocks_exclusive_upgrade_while_another_profile_holds_shared() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("exec.lock");

        let mut first = ExecutableLock::acquire_shared(lock_path.clone()).unwrap();
        let second = ExecutableLock::acquire_shared(lock_path.clone()).unwrap();
        assert!(!first.try_upgrade_exclusive().unwrap());

        // `first` must have re-acquired its shared lock after failing to upgrade.
        let mut probe = ExecutableLock::acquire_shared(lock_path).unwrap();
        drop(second);
        assert!(!first.try_upgrade_exclusive().unwrap());
        assert!(!probe.try_upgrade_exclusive().unwrap());
        drop(probe);

        assert!(first.try_upgrade_exclusive().unwrap());
        first.downgrade_to_shared();
        drop(first);
    }

    #[test]
    fn windows_installer_staging_writes_executable_and_prunes_stale_directories() {
        let root = tempfile::tempdir().unwrap();
        let keep_unrelated = root.path().join("unrelated-dir");
        fs::create_dir(&keep_unrelated).unwrap();
        assert!(stage_windows_installer_in(b"not-pe", root.path()).is_err());

        let (installer, stage_lock) =
            stage_windows_installer_in(b"MZ\x90\x00dummy", root.path()).unwrap();
        assert!(installer.is_file());
        cleanup_stale_installers_in(root.path());
        assert!(
            installer.is_file(),
            "held stage lock must protect pending installer"
        );
        drop(stage_lock);
        cleanup_stale_installers_in(root.path());
        assert!(!installer.parent().unwrap().exists());
        assert!(keep_unrelated.exists());
    }
}
