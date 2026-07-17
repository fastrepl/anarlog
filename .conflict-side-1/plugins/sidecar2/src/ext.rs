use tauri::Manager;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::Command;

use crate::Error;

pub struct Sidecar2<'a, R: tauri::Runtime, M: Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: Manager<R>> Sidecar2<'a, R, M> {
    pub fn sidecar(&self, name: impl AsRef<str>) -> Result<Command, Error> {
        let name = name.as_ref();
        let home_dir = dirs::home_dir().unwrap();

        #[cfg(debug_assertions)]
        {
            if let Some(binary_name) = name.strip_prefix("char-sidecar-") {
                let (passthrough, binary) =
                    resolve_debug_paths(binary_name).ok_or(Error::BinaryNotFound)?;
                return Ok(self
                    .manager
                    .shell()
                    .command(&passthrough)
                    .current_dir(home_dir)
                    .arg(&binary));
            }
        }

        #[cfg(not(debug_assertions))]
        {
            match self.manager.shell().sidecar(name) {
                Ok(cmd) => return Ok(cmd.current_dir(home_dir)),
                Err(e) => {
                    if is_symlink_launch() {
                        let sidecar_path = resolve_sidecar_for_symlink_launch(name)?;
                        return Ok(self
                            .manager
                            .shell()
                            .command(&sidecar_path)
                            .current_dir(home_dir));
                    }
                    return Err(Error::SidecarCreationFailed(e.to_string()));
                }
            }
        }

        #[cfg(debug_assertions)]
        {
            Ok(self
                .manager
                .shell()
                .sidecar(name)
                .map_err(|e| Error::SidecarCreationFailed(e.to_string()))?
                .current_dir(home_dir))
        }
    }
}

#[cfg(debug_assertions)]
fn resolve_debug_paths(binary_name: &str) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let passthrough = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src-tauri/resources/passthrough-aarch64-apple-darwin");
    let binary = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../apps/desktop/src-tauri/resources/{}-aarch64-apple-darwin",
        binary_name
    ));

    if passthrough.exists() && binary.exists() {
        Some((passthrough, binary))
    } else {
        None
    }
}

#[cfg(not(debug_assertions))]
fn is_symlink_launch() -> bool {
    let Ok(exe_path) = std::env::current_exe() else {
        return false;
    };

    let Ok(metadata) = std::fs::symlink_metadata(&exe_path) else {
        return false;
    };

    metadata.file_type().is_symlink()
}

#[cfg(not(debug_assertions))]
fn resolve_sidecar_for_symlink_launch(name: &str) -> Result<std::path::PathBuf, crate::Error> {
    let exe_path = std::env::current_exe().map_err(|e| {
        crate::Error::SidecarCreationFailed(format!("failed to get current exe: {}", e))
    })?;

    let resolved_exe = std::fs::canonicalize(&exe_path).map_err(|e| {
        crate::Error::SidecarCreationFailed(format!("failed to resolve symlink: {}", e))
    })?;

    let exe_dir = resolved_exe.parent().ok_or_else(|| {
        crate::Error::SidecarCreationFailed("failed to get exe parent directory".to_string())
    })?;

    let sidecar_path = std::fs::read_dir(exe_dir)
        .map_err(|e| {
            crate::Error::SidecarCreationFailed(format!("failed to read exe directory: {}", e))
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(&format!("{}-", name)))
                .unwrap_or(false)
        })
        .ok_or_else(|| {
            crate::Error::SidecarCreationFailed(format!(
                "sidecar '{}' not found in {}",
                name,
                exe_dir.display()
            ))
        })?;

    let metadata = std::fs::symlink_metadata(&sidecar_path).map_err(|e| {
        crate::Error::SidecarCreationFailed(format!("failed to get sidecar metadata: {}", e))
    })?;

    if metadata.file_type().is_symlink() {
        return Err(crate::Error::SidecarCreationFailed(
            "sidecar binary is a symlink".to_string(),
        ));
    }

    Ok(sidecar_path)
}

pub trait Sidecar2PluginExt<R: tauri::Runtime> {
    fn sidecar2(&self) -> Sidecar2<'_, R, Self>
    where
        Self: Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: Manager<R>> Sidecar2PluginExt<R> for T {
    fn sidecar2(&self) -> Sidecar2<'_, R, Self>
    where
        Self: Sized,
    {
        Sidecar2 {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
