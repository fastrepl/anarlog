use std::{
    fs,
    io::{Cursor, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};

use desktop_runtime::{
    CancellationToken, Result, RuntimeHandle, ServiceError,
    updater::{InstallConditions, NativeUpdater, UpdateInstaller, VerifiedUpdate},
};
use futures::future::BoxFuture;

#[derive(Clone, Copy)]
pub enum PackageKind {
    AppImage,
    MacBundle,
    WindowsNsis,
    WindowsMsi,
}

#[derive(Clone)]
pub struct NativeInstaller {
    pub kind: PackageKind,
    pub installation: PathBuf,
    pub staging_root: PathBuf,
}

pub async fn download_and_install(
    updater: &NativeUpdater,
    runtime: &RuntimeHandle,
    release: desktop_runtime::updater::ResolvedRelease,
    installer: &NativeInstaller,
    conditions: impl Fn() -> InstallConditions,
    cancel: CancellationToken,
) -> Result<()> {
    NativeUpdater::permit_install(conditions())?;
    let update = updater
        .download(runtime, release, cancel.clone())?
        .receive()
        .await?;
    if cancel.is_cancelled() {
        return Err(ServiceError::Cancelled);
    }
    NativeUpdater::permit_install(conditions())?;
    installer.install(update).await
}

impl UpdateInstaller for NativeInstaller {
    fn install(&self, update: VerifiedUpdate) -> BoxFuture<'static, Result<()>> {
        let installer = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || installer.install_verified(update.bytes()))
                .await
                .map_err(failure)?
        })
    }
}

impl NativeInstaller {
    fn install_verified(&self, bytes: &[u8]) -> Result<()> {
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
            PackageKind::MacBundle => {
                if !cfg!(target_os = "macos") {
                    return Err(failure("App bundle installation requires macOS"));
                }
                let parent = self
                    .installation
                    .parent()
                    .ok_or_else(|| failure("Missing bundle parent"))?;
                let stage = tempfile::tempdir_in(parent).map_err(failure)?;
                let package = extract_single_package(bytes, stage.path(), true)?;
                if !Command::new("/usr/bin/codesign")
                    .args(["--verify", "--deep", "--strict"])
                    .arg(&package)
                    .status()
                    .map_err(failure)?
                    .success()
                {
                    return Err(failure("Bundle code-signature validation failed"));
                }
                replace_with_backup(&package, &self.installation)?;
            }
            PackageKind::WindowsNsis | PackageKind::WindowsMsi => {
                if !cfg!(target_os = "windows") {
                    return Err(failure("Windows installer requires Windows"));
                }
                fs::create_dir_all(&self.staging_root).map_err(failure)?;
                let nsis = matches!(self.kind, PackageKind::WindowsNsis);
                if nsis && !bytes.starts_with(b"MZ") {
                    return Err(failure("Expected a signed NSIS executable"));
                }
                if !nsis && !bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0]) {
                    return Err(failure("Expected an MSI package"));
                }
                let mut file = tempfile::Builder::new()
                    .suffix(if nsis { ".exe" } else { ".msi" })
                    .tempfile_in(&self.staging_root)
                    .map_err(failure)?;
                file.write_all(bytes).map_err(failure)?;
                file.as_file().sync_all().map_err(failure)?;
                let (_, path) = file.keep().map_err(failure)?;
                let result = if nsis {
                    Command::new(&path).arg("/UPDATE").spawn()
                } else {
                    Command::new("msiexec.exe")
                        .arg("/i")
                        .arg(&path)
                        .arg("/passive")
                        .spawn()
                };
                if let Err(error) = result {
                    fs::remove_file(&path).map_err(failure)?;
                    return Err(failure(error));
                }
            }
        }
        Ok(())
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

fn replace_with_backup(package: &Path, installation: &Path) -> Result<PathBuf> {
    let backup = installation.with_extension(format!("previous-{}", uuid::Uuid::new_v4()));
    fs::rename(installation, &backup).map_err(failure)?;
    if let Err(error) = fs::rename(package, installation) {
        fs::rename(&backup, installation).map_err(failure)?;
        return Err(failure(error));
    }
    Ok(backup)
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_retains_rollback_and_rejects_unsafe_archive_paths() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("app");
        let new = root.path().join("new");
        fs::write(&old, b"old").unwrap();
        fs::write(&new, b"new").unwrap();
        let backup = replace_with_backup(&new, &old).unwrap();
        assert_eq!(fs::read(&old).unwrap(), b"new");
        assert_eq!(fs::read(backup).unwrap(), b"old");
        assert!(!relative_path(Path::new("../outside")));
        assert!(!relative_path(Path::new("/absolute")));
        assert!(relative_path(Path::new("App.app/Contents/Resources")));
        assert!(replace_with_backup(&root.path().join("missing"), &old).is_err());
        assert_eq!(fs::read(&old).unwrap(), b"new");
    }
}
