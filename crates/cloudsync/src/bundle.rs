use std::fs;
use std::path::PathBuf;

use crate::CLOUDSYNC_VERSION;
use crate::error::Error;

macro_rules! configure_cloudsync_target {
    ($target:literal, $file_name:literal, $path:literal) => {
        const CLOUDSYNC_TARGET: &str = $target;
        const CLOUDSYNC_FILE_NAME: &str = $file_name;
        const BUNDLED_CLOUDSYNC_BYTES: &[u8] = include_bytes!($path);
    };
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
configure_cloudsync_target!(
    "macos/aarch64",
    "cloudsync.dylib",
    "../vendor/cloudsync/macos/aarch64/cloudsync.dylib"
);

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
configure_cloudsync_target!(
    "macos/x86_64",
    "cloudsync.dylib",
    "../vendor/cloudsync/macos/x86_64/cloudsync.dylib"
);

#[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"))]
configure_cloudsync_target!(
    "linux/gnu/aarch64",
    "cloudsync.so",
    "../vendor/cloudsync/linux/gnu/aarch64/cloudsync.so"
);

#[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]
configure_cloudsync_target!(
    "linux/gnu/x86_64",
    "cloudsync.so",
    "../vendor/cloudsync/linux/gnu/x86_64/cloudsync.so"
);

#[cfg(all(target_os = "linux", target_env = "musl", target_arch = "aarch64"))]
configure_cloudsync_target!(
    "linux/musl/aarch64",
    "cloudsync.so",
    "../vendor/cloudsync/linux/musl/aarch64/cloudsync.so"
);

#[cfg(all(target_os = "linux", target_env = "musl", target_arch = "x86_64"))]
configure_cloudsync_target!(
    "linux/musl/x86_64",
    "cloudsync.so",
    "../vendor/cloudsync/linux/musl/x86_64/cloudsync.so"
);

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
configure_cloudsync_target!(
    "windows/x86_64",
    "cloudsync.dll",
    "../vendor/cloudsync/windows/x86_64/cloudsync.dll"
);

pub fn bundled_extension_path() -> Result<PathBuf, Error> {
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    {
        Err(Error::UnsupportedBundledCloudsync)
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
        all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
        all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    {
        let base_dir = dirs::cache_dir()
            .ok_or(Error::MissingCacheDir)?
            .join("char")
            .join("cloudsync")
            .join(CLOUDSYNC_VERSION)
            .join(CLOUDSYNC_TARGET);

        fs::create_dir_all(&base_dir)?;

        let extension_path = base_dir.join(CLOUDSYNC_FILE_NAME);
        let needs_write = match fs::metadata(&extension_path) {
            Ok(metadata) => metadata.len() != BUNDLED_CLOUDSYNC_BYTES.len() as u64,
            Err(_) => true,
        };

        if needs_write {
            let tmp_path =
                base_dir.join(format!("{CLOUDSYNC_FILE_NAME}.{}.tmp", std::process::id()));
            fs::write(&tmp_path, BUNDLED_CLOUDSYNC_BYTES)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755))?;
            }

            match fs::rename(&tmp_path, &extension_path) {
                Ok(()) => {}
                Err(error) if extension_path.exists() => {
                    let _ = fs::remove_file(&tmp_path);

                    if fs::metadata(&extension_path)?.len() != BUNDLED_CLOUDSYNC_BYTES.len() as u64
                    {
                        return Err(error.into());
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }

        Ok(extension_path)
    }
}
