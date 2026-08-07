#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::os::unix::fs::PermissionsExt;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use std::path::Path;
use std::path::PathBuf;

use serde::Serialize;

const DEV_BUNDLE_ID: &str = "com.hyprnote.dev";
const FLATPAK_BUNDLE_ID: &str = "so.anarlog.Anarlog";
const LEGACY_STABLE_BUNDLE_ID: &str = "com.hyprnote.Hyprnote";
#[cfg(any(target_os = "macos", target_os = "linux"))]
const MANAGED_CLI_DIR: &str = ".anarlog-cli";
const STABLE_BUNDLE_ID: &str = "com.hyprnote.stable";
const STAGING_BUNDLE_ID: &str = "com.hyprnote.staging";

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedCliState {
    Installed,
    Missing,
    Conflict,
    Unsupported,
    ResourceMissing,
}

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedCliStatus {
    pub supported: bool,
    pub command_name: String,
    pub install_path: String,
    pub state: EmbeddedCliState,
    pub details: Option<String>,
}

pub fn check<R: tauri::Runtime, T: tauri::Manager<R>>(manager: &T) -> EmbeddedCliStatus {
    let command_name = command_name_from_identifier(manager.config().identifier.as_ref());
    let Some(install_path) = install_path_for_command(command_name) else {
        // Windows resolves the install path from local app data, not the home
        // directory, so the two platforms cannot share one message.
        #[cfg(target_os = "windows")]
        let missing_dir = "Anarlog could not find your local application data directory.";
        #[cfg(not(target_os = "windows"))]
        let missing_dir = "Anarlog could not find your home directory.";

        return unavailable_status(command_name, missing_dir);
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = manager;
        return EmbeddedCliStatus {
            supported: false,
            command_name: command_name.to_string(),
            install_path: install_path.display().to_string(),
            state: EmbeddedCliState::Unsupported,
            details: Some(
                "Bundled CLI installation is not yet available on this platform.".to_string(),
            ),
        };
    }

    #[cfg(target_os = "windows")]
    {
        let Some(_resource_path) = resolve_resource_path(manager) else {
            return EmbeddedCliStatus {
                supported: true,
                command_name: command_name.to_string(),
                install_path: install_path.display().to_string(),
                state: EmbeddedCliState::ResourceMissing,
                details: Some("The CLI is not included in this build of Anarlog.".to_string()),
            };
        };

        classify_windows_status(command_name, install_path)
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let Some(_resource_path) = resolve_resource_path(manager) else {
            return EmbeddedCliStatus {
                supported: true,
                command_name: command_name.to_string(),
                install_path: install_path.display().to_string(),
                state: EmbeddedCliState::ResourceMissing,
                details: Some("The CLI is not included in this build of Anarlog.".to_string()),
            };
        };
        let app_version = manager.package_info().version.to_string();

        classify_status(command_name, install_path, &app_version)
    }
}

pub fn install<R: tauri::Runtime, T: tauri::Manager<R>>(
    manager: &T,
) -> Result<EmbeddedCliStatus, String> {
    let status = check(manager);

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Ok(status)
    }

    #[cfg(target_os = "windows")]
    {
        match status.state {
            EmbeddedCliState::Unsupported | EmbeddedCliState::ResourceMissing => {
                return Ok(status);
            }
            EmbeddedCliState::Conflict => {
                return Err(format!(
                    "Another file already exists at {}. Move it before installing the Anarlog CLI.",
                    status.install_path
                ));
            }
            EmbeddedCliState::Installed | EmbeddedCliState::Missing => {}
        }

        let resource_path = resolve_resource_path(manager)
            .ok_or_else(|| "The bundled CLI could not be found.".to_string())?;
        let install_path = PathBuf::from(&status.install_path);
        install_windows_cli(&resource_path, &install_path)?;
        add_windows_cli_to_path(&install_path)?;
        Ok(classify_windows_status(&status.command_name, install_path))
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        match status.state {
            EmbeddedCliState::Unsupported | EmbeddedCliState::ResourceMissing => {
                return Ok(status);
            }
            EmbeddedCliState::Conflict => {
                return Err(format!(
                    "Another file already exists at {}. Move it before installing the Anarlog CLI.",
                    status.install_path
                ));
            }
            EmbeddedCliState::Installed | EmbeddedCliState::Missing => {}
        }

        let resource_path = resolve_resource_path(manager)
            .ok_or_else(|| "The bundled CLI could not be found.".to_string())?;
        let install_path = PathBuf::from(&status.install_path);
        let app_version = manager.package_info().version.to_string();
        let managed_path = managed_binary_path(&install_path, &status.command_name, &app_version)?;

        install_managed_cli(&resource_path, &managed_path, &install_path)?;
        Ok(classify_status(
            &status.command_name,
            install_path,
            &app_version,
        ))
    }
}

fn unavailable_status(command_name: &str, details: &str) -> EmbeddedCliStatus {
    EmbeddedCliStatus {
        supported: false,
        command_name: command_name.to_string(),
        install_path: String::new(),
        state: EmbeddedCliState::Unsupported,
        details: Some(details.to_string()),
    }
}

fn command_name_from_identifier(identifier: &str) -> &'static str {
    match identifier {
        STABLE_BUNDLE_ID | LEGACY_STABLE_BUNDLE_ID | FLATPAK_BUNDLE_ID => "anarlog",
        STAGING_BUNDLE_ID => "anarlog-staging",
        DEV_BUNDLE_ID => "anarlog-dev",
        _ => "anarlog",
    }
}

fn install_path_for_command(command_name: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return dirs::data_local_dir().map(|data_dir| {
            data_dir
                .join("Anarlog")
                .join("bin")
                .join(format!("{command_name}.exe"))
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir().map(|home| home.join(".local/bin").join(command_name))
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn resolve_resource_path<R: tauri::Runtime, T: tauri::Manager<R>>(manager: &T) -> Option<PathBuf> {
    use tauri::path::BaseDirectory;

    if let Some(sidecar_path) = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join(sidecar_binary_name()))
        })
        .filter(|path| path.is_file())
    {
        return Some(sidecar_path);
    }

    let file_name = bundled_binary_name()?;

    if let Some(bundled_resource_path) = manager
        .path()
        .resolve(format!("cli/{file_name}"), BaseDirectory::Resource)
        .ok()
        .filter(|path| path.exists())
    {
        return Some(bundled_resource_path);
    }

    let debug_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("cli")
        .join(file_name);
    debug_path.exists().then_some(debug_path)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn sidecar_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "anarlog-cli.exe"
    } else {
        "anarlog-cli"
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn bundled_binary_name() -> Option<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Some("anarlog-cli-x86_64-pc-windows-msvc.exe");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Some("anarlog-cli-aarch64-apple-darwin");
    }

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Some("anarlog-cli-x86_64-apple-darwin");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Some("anarlog-cli-x86_64-unknown-linux-gnu");
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Some("anarlog-cli-aarch64-unknown-linux-gnu");
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(target_os = "windows")]
fn classify_windows_status(command_name: &str, install_path: PathBuf) -> EmbeddedCliStatus {
    let state = match std::fs::symlink_metadata(&install_path) {
        Ok(metadata) if metadata.is_file() => {
            // Reporting Conflict here would disable Install/Reinstall in settings,
            // even though re-running the install is what repairs a missing or
            // unreadable PATH entry.
            let on_path = install_path
                .parent()
                .ok_or_else(|| "The CLI install directory is invalid.".to_string())
                .and_then(windows_path_contains)
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "failed_to_check_windows_cli_path");
                    false
                });

            Ok(if on_path {
                EmbeddedCliState::Installed
            } else {
                EmbeddedCliState::Missing
            })
        }
        Ok(_) => Ok(EmbeddedCliState::Conflict),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(EmbeddedCliState::Missing),
        Err(error) => Err(format!(
            "Failed to inspect {}: {error}",
            install_path.display()
        )),
    };

    match state {
        Ok(state) => EmbeddedCliStatus {
            supported: true,
            command_name: command_name.to_string(),
            install_path: install_path.display().to_string(),
            state,
            details: windows_details_for_state(state, &install_path),
        },
        Err(error) => EmbeddedCliStatus {
            supported: true,
            command_name: command_name.to_string(),
            install_path: install_path.display().to_string(),
            state: EmbeddedCliState::Conflict,
            details: Some(error),
        },
    }
}

#[cfg(target_os = "windows")]
fn windows_details_for_state(state: EmbeddedCliState, install_path: &Path) -> Option<String> {
    match state {
        EmbeddedCliState::Installed => Some(format!(
            "Installed at {} and available in new terminals.",
            install_path.display()
        )),
        EmbeddedCliState::Missing => Some(format!(
            "Install the command at {} and add it to your user PATH.",
            install_path.display()
        )),
        EmbeddedCliState::Conflict => Some(format!(
            "Another file already exists at {}.",
            install_path.display()
        )),
        EmbeddedCliState::Unsupported | EmbeddedCliState::ResourceMissing => None,
    }
}

#[cfg(target_os = "windows")]
fn install_windows_cli(resource_path: &Path, install_path: &Path) -> Result<(), String> {
    let install_dir = install_path
        .parent()
        .ok_or_else(|| "The CLI install directory is invalid.".to_string())?;
    std::fs::create_dir_all(install_dir)
        .map_err(|error| format!("Could not create {}: {error}", install_dir.display()))?;

    let file_name = install_path
        .file_name()
        .ok_or_else(|| "The CLI install path is invalid.".to_string())?;
    let temp_path = install_path.with_file_name(format!(
        ".{}.tmp-{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    if std::fs::symlink_metadata(&temp_path).is_ok() {
        std::fs::remove_file(&temp_path).map_err(|error| {
            format!(
                "Could not prepare the CLI update at {}: {error}",
                temp_path.display()
            )
        })?;
    }

    std::fs::copy(resource_path, &temp_path).map_err(|error| {
        format!(
            "Could not copy the bundled CLI to {}: {error}",
            temp_path.display()
        )
    })?;
    // Windows refuses to delete a running executable but still allows renaming it, so
    // the old binary is moved aside rather than removed. Anything still running keeps
    // its handle to the backup, which the next install clears once it is released.
    let backup_path = install_path.with_file_name(format!(".{}.old", file_name.to_string_lossy()));
    let _ = std::fs::remove_file(&backup_path);

    let replaced = install_path.exists();
    if replaced {
        std::fs::rename(install_path, &backup_path).map_err(|error| {
            let _ = std::fs::remove_file(&temp_path);
            format!(
                "Could not replace the CLI at {}: {error}",
                install_path.display()
            )
        })?;
    }

    if let Err(error) = std::fs::rename(&temp_path, install_path) {
        let _ = std::fs::remove_file(&temp_path);
        if replaced {
            let _ = std::fs::rename(&backup_path, install_path);
        }
        return Err(format!(
            "Could not install the CLI at {}: {error}",
            install_path.display()
        ));
    }

    let _ = std::fs::remove_file(&backup_path);
    Ok(())
}

// Treating an unreadable Path as empty would let the caller overwrite the user PATH
// with only the install directory, so only a genuinely absent value reads as empty.
#[cfg(target_os = "windows")]
fn read_user_path(environment: &windows_registry::Key) -> Result<String, String> {
    match environment.get_string("Path") {
        Ok(path) => Ok(path),
        Err(_) if environment.get_type("Path").is_err() => Ok(String::new()),
        Err(error) => Err(format!("Could not read the user PATH: {error}")),
    }
}

#[cfg(target_os = "windows")]
fn windows_path_contains(install_dir: &Path) -> Result<bool, String> {
    let environment = windows_registry::CURRENT_USER
        .open("Environment")
        .map_err(|error| format!("Could not read the user environment: {error}"))?;
    let path = read_user_path(&environment)?;
    Ok(path_list_contains(&path, install_dir))
}

#[cfg(target_os = "windows")]
fn add_windows_cli_to_path(install_path: &Path) -> Result<(), String> {
    let install_dir = install_path
        .parent()
        .ok_or_else(|| "The CLI install directory is invalid.".to_string())?;
    let environment = windows_registry::CURRENT_USER
        .create("Environment")
        .map_err(|error| format!("Could not open the user environment: {error}"))?;
    let path = read_user_path(&environment)?;
    if path_list_contains(&path, install_dir) {
        return Ok(());
    }

    let updated = if path.trim().is_empty() {
        install_dir.display().to_string()
    } else {
        format!("{};{}", path.trim_end_matches(';'), install_dir.display())
    };
    let result = if environment.get_type("Path") == Ok(windows_registry::Type::ExpandString) {
        environment
            .set_expand_string("Path", updated)
            .map_err(|error| format!("Could not update the user PATH: {error}"))
    } else {
        environment
            .set_string("Path", updated)
            .map_err(|error| format!("Could not update the user PATH: {error}"))
    };
    result?;
    notify_windows_environment_changed();
    Ok(())
}

#[cfg(target_os = "windows")]
fn notify_windows_environment_changed() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };

    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(windows::core::w!("Environment").as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5_000,
            None,
        );
    }
}

#[cfg(any(test, target_os = "windows"))]
fn path_list_contains(path_list: &str, expected: &Path) -> bool {
    let expected = expected
        .to_string_lossy()
        .trim_matches('"')
        .trim_end_matches(['\\', '/'])
        .to_string();
    path_list.split(';').any(|entry| {
        entry
            .trim()
            .trim_matches('"')
            .trim_end_matches(['\\', '/'])
            .eq_ignore_ascii_case(&expected)
    })
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn classify_status(
    command_name: &str,
    install_path: PathBuf,
    app_version: &str,
) -> EmbeddedCliStatus {
    let state = managed_binary_path(&install_path, command_name, app_version)
        .and_then(|managed_path| classify_installation(&install_path, &managed_path));

    match state {
        Ok(state) => EmbeddedCliStatus {
            supported: true,
            command_name: command_name.to_string(),
            install_path: install_path.display().to_string(),
            state,
            details: details_for_state(state, &install_path),
        },
        Err(error) => EmbeddedCliStatus {
            supported: true,
            command_name: command_name.to_string(),
            install_path: install_path.display().to_string(),
            state: EmbeddedCliState::Conflict,
            details: Some(error),
        },
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn classify_installation(
    install_path: &Path,
    managed_path: &Path,
) -> Result<EmbeddedCliState, String> {
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(EmbeddedCliState::Missing);
        }
        Err(error) => {
            return Err(format!(
                "Failed to inspect {}: {error}",
                install_path.display()
            ));
        }
    };

    if !metadata.file_type().is_symlink() {
        return Ok(EmbeddedCliState::Conflict);
    }

    let installed_target = std::fs::read_link(install_path).map_err(|error| {
        format!(
            "Failed to inspect the installed command at {}: {error}",
            install_path.display()
        )
    })?;
    if !is_replaceable_symlink_target(&installed_target, managed_path) {
        return Ok(EmbeddedCliState::Conflict);
    }
    if installed_target != managed_path {
        return Ok(EmbeddedCliState::Missing);
    }

    let managed_metadata = match std::fs::symlink_metadata(managed_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(EmbeddedCliState::Missing);
        }
        Err(error) => {
            return Err(format!(
                "Failed to resolve the managed CLI at {}: {error}",
                managed_path.display()
            ));
        }
    };

    if !managed_metadata.file_type().is_file() || managed_metadata.permissions().mode() & 0o100 == 0
    {
        return Ok(EmbeddedCliState::Missing);
    }

    Ok(EmbeddedCliState::Installed)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn is_replaceable_symlink_target(target: &Path, managed_path: &Path) -> bool {
    if managed_path
        .parent()
        .is_some_and(|managed_dir| target.parent() == Some(managed_dir))
    {
        return true;
    }

    #[cfg(target_os = "macos")]
    {
        return is_legacy_app_cli_target(target);
    }

    #[cfg(target_os = "linux")]
    false
}

#[cfg(target_os = "macos")]
fn is_legacy_app_cli_target(target: &Path) -> bool {
    let Some(file_name) = target.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !matches!(
        file_name,
        "anarlog-cli" | "anarlog-cli-aarch64-apple-darwin" | "anarlog-cli-x86_64-apple-darwin"
    ) {
        return false;
    }

    let Some(parent) = target.parent() else {
        return false;
    };
    let contents_dir = match parent.file_name().and_then(|name| name.to_str()) {
        Some("MacOS") | Some("Resources") => parent.parent(),
        Some("cli") => parent
            .parent()
            .filter(|path| path.file_name().is_some_and(|name| name == "Resources"))
            .and_then(Path::parent),
        _ => None,
    };
    let Some(app_name) = contents_dir
        .filter(|path| path.file_name().is_some_and(|name| name == "Contents"))
        .and_then(|path| path.parent())
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
    else {
        return false;
    };

    matches!(
        app_name,
        "Anarlog.app" | "Anarlog Staging.app" | "Anarlog Dev.app"
    )
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn details_for_state(state: EmbeddedCliState, install_path: &Path) -> Option<String> {
    match state {
        EmbeddedCliState::Installed => Some(format!(
            "Installed at {} and managed by Anarlog.",
            install_path.display()
        )),
        EmbeddedCliState::Missing => Some(format!(
            "Install the command at {}.",
            install_path.display()
        )),
        EmbeddedCliState::Conflict => Some(format!(
            "Another file already exists at {}.",
            install_path.display()
        )),
        EmbeddedCliState::Unsupported => None,
        EmbeddedCliState::ResourceMissing => None,
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn managed_binary_path(
    install_path: &Path,
    command_name: &str,
    app_version: &str,
) -> Result<PathBuf, String> {
    let install_dir = install_path
        .parent()
        .ok_or_else(|| "The CLI install directory is invalid.".to_string())?;

    Ok(install_dir
        .join(MANAGED_CLI_DIR)
        .join(command_name)
        .join(app_version))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn install_managed_cli(
    resource_path: &Path,
    managed_path: &Path,
    install_path: &Path,
) -> Result<(), String> {
    let managed_dir = managed_path
        .parent()
        .ok_or_else(|| "The managed CLI directory is invalid.".to_string())?;
    std::fs::create_dir_all(managed_dir)
        .map_err(|error| format!("Could not create {}: {error}", managed_dir.display()))?;

    let file_name = managed_path
        .file_name()
        .ok_or_else(|| "The managed CLI path is invalid.".to_string())?;
    let temp_path = managed_path.with_file_name(format!(
        ".{}.tmp-{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    if std::fs::symlink_metadata(&temp_path).is_ok() {
        std::fs::remove_file(&temp_path).map_err(|error| {
            format!(
                "Could not prepare the CLI update at {}: {error}",
                temp_path.display()
            )
        })?;
    }

    std::fs::copy(resource_path, &temp_path).map_err(|error| {
        format!(
            "Could not copy the bundled CLI to {}: {error}",
            temp_path.display()
        )
    })?;
    let mut permissions = std::fs::metadata(&temp_path)
        .map_err(|error| {
            format!(
                "Could not inspect the CLI update at {}: {error}",
                temp_path.display()
            )
        })?
        .permissions();
    permissions.set_mode(permissions.mode() | 0o100);
    if let Err(error) = std::fs::set_permissions(&temp_path, permissions) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Could not make the CLI executable at {}: {error}",
            temp_path.display()
        ));
    }
    if let Err(error) = std::fs::rename(&temp_path, managed_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Could not install the managed CLI at {}: {error}",
            managed_path.display()
        ));
    }

    install_symlink(managed_path, install_path)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn install_symlink(managed_path: &Path, install_path: &Path) -> Result<(), String> {
    let install_dir = install_path
        .parent()
        .ok_or_else(|| "The CLI install directory is invalid.".to_string())?;
    std::fs::create_dir_all(install_dir)
        .map_err(|error| format!("Could not create {}: {error}", install_dir.display()))?;

    let file_name = install_path
        .file_name()
        .ok_or_else(|| "The CLI install path is invalid.".to_string())?;
    let temp_path = install_path.with_file_name(format!(
        ".{}.tmp-{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    if std::fs::symlink_metadata(&temp_path).is_ok() {
        std::fs::remove_file(&temp_path).map_err(|error| {
            format!(
                "Could not prepare the command update at {}: {error}",
                temp_path.display()
            )
        })?;
    }

    std::os::unix::fs::symlink(managed_path, &temp_path).map_err(|error| {
        format!(
            "Could not prepare the command at {}: {error}",
            temp_path.display()
        )
    })?;
    if let Err(error) = ensure_install_path_replaceable(install_path, managed_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temp_path, install_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Could not install the command at {}: {error}",
            install_path.display()
        ));
    }

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn ensure_install_path_replaceable(install_path: &Path, managed_path: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to inspect {}: {error}",
                install_path.display()
            ));
        }
    };

    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(install_path).map_err(|error| {
            format!(
                "Failed to inspect the installed command at {}: {error}",
                install_path.display()
            )
        })?;
        if is_replaceable_symlink_target(&target, managed_path) {
            return Ok(());
        }
    }

    Err(format!(
        "Another file already exists at {}.",
        install_path.display()
    ))
}

#[cfg(test)]
mod tests;
