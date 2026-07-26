#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;

use serde::Serialize;

const DEV_BUNDLE_ID: &str = "com.hyprnote.dev";
#[cfg(target_os = "macos")]
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
        return unavailable_status(command_name, "Anarlog could not find your home directory.");
    };

    #[cfg(not(target_os = "macos"))]
    {
        let _ = manager;
        return EmbeddedCliStatus {
            supported: false,
            command_name: command_name.to_string(),
            install_path: install_path.display().to_string(),
            state: EmbeddedCliState::Unsupported,
            details: Some("Bundled CLI installation is currently available on macOS.".to_string()),
        };
    }

    #[cfg(target_os = "macos")]
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

    #[cfg(not(target_os = "macos"))]
    {
        Ok(status)
    }

    #[cfg(target_os = "macos")]
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
        STABLE_BUNDLE_ID => "anarlog",
        STAGING_BUNDLE_ID => "anarlog-staging",
        DEV_BUNDLE_ID => "anarlog-dev",
        _ => "anarlog-dev",
    }
}

fn install_path_for_command(command_name: &str) -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".local/bin").join(command_name))
}

#[cfg(target_os = "macos")]
fn resolve_resource_path<R: tauri::Runtime, T: tauri::Manager<R>>(manager: &T) -> Option<PathBuf> {
    use tauri::path::BaseDirectory;

    if let Some(sidecar_path) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("anarlog-cli")))
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

#[cfg(target_os = "macos")]
fn bundled_binary_name() -> Option<&'static str> {
    #[cfg(target_arch = "aarch64")]
    {
        return Some("anarlog-cli-aarch64-apple-darwin");
    }

    #[cfg(target_arch = "x86_64")]
    {
        return Some("anarlog-cli-x86_64-apple-darwin");
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn is_replaceable_symlink_target(target: &Path, managed_path: &Path) -> bool {
    managed_path
        .parent()
        .is_some_and(|managed_dir| target.parent() == Some(managed_dir))
        || is_legacy_app_cli_target(target)
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn maps_bundle_id_to_command_name() {
        assert_eq!(command_name_from_identifier(STABLE_BUNDLE_ID), "anarlog");
        assert_eq!(
            command_name_from_identifier(STAGING_BUNDLE_ID),
            "anarlog-staging"
        );
        assert_eq!(command_name_from_identifier(DEV_BUNDLE_ID), "anarlog-dev");
        assert_eq!(command_name_from_identifier("unknown"), "anarlog-dev");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_missing_install() {
        let dir = tempfile::tempdir().unwrap();
        let resource_path = dir.path().join("anarlog-cli");
        std::fs::write(&resource_path, "cli").unwrap();

        let state = classify_installation(&dir.path().join("anarlog"), &resource_path).unwrap();
        assert_eq!(state, EmbeddedCliState::Missing);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_installed_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let managed_path = dir.path().join("managed-anarlog-cli");
        std::fs::write(&managed_path, "cli").unwrap();
        std::fs::set_permissions(&managed_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let install_path = dir.path().join("anarlog");
        std::os::unix::fs::symlink(&managed_path, &install_path).unwrap();

        let state = classify_installation(&install_path, &managed_path).unwrap();
        assert_eq!(state, EmbeddedCliState::Installed);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_non_executable_managed_cli_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        let managed_path = dir.path().join("managed-anarlog-cli");
        std::fs::write(&managed_path, "cli").unwrap();
        std::fs::set_permissions(&managed_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let install_path = dir.path().join("anarlog");
        std::os::unix::fs::symlink(&managed_path, &install_path).unwrap();

        assert_eq!(
            classify_installation(&install_path, &managed_path).unwrap(),
            EmbeddedCliState::Missing
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_stale_symlinks_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        let managed_path = dir.path().join("anarlog-cli");
        let old_managed_path = dir.path().join("old-anarlog-cli");
        let install_path = dir.path().join("anarlog");
        std::fs::write(&managed_path, "new cli").unwrap();
        std::fs::write(&old_managed_path, "old cli").unwrap();
        std::os::unix::fs::symlink(&old_managed_path, &install_path).unwrap();

        assert_eq!(
            classify_installation(&install_path, &managed_path).unwrap(),
            EmbeddedCliState::Missing
        );

        std::fs::remove_file(old_managed_path).unwrap();
        assert_eq!(
            classify_installation(&install_path, &managed_path).unwrap(),
            EmbeddedCliState::Missing
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_legacy_app_resource_symlink_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        let managed_path = dir.path().join("managed-anarlog-cli");
        let app_resource_path = dir
            .path()
            .join("Anarlog.app/Contents/Resources/anarlog-cli");
        let install_path = dir.path().join("anarlog");
        std::fs::create_dir_all(app_resource_path.parent().unwrap()).unwrap();
        std::fs::write(&app_resource_path, "cli").unwrap();
        std::os::unix::fs::symlink(&app_resource_path, &install_path).unwrap();

        assert_eq!(
            classify_installation(&install_path, &managed_path).unwrap(),
            EmbeddedCliState::Missing
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_legacy_app_executable_symlink_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        let managed_path = dir.path().join(".anarlog-cli/anarlog/1.2.0");
        let app_executable_path = dir.path().join("Anarlog.app/Contents/MacOS/anarlog-cli");
        let install_path = dir.path().join("anarlog");
        std::fs::create_dir_all(app_executable_path.parent().unwrap()).unwrap();
        std::fs::write(&app_executable_path, "cli").unwrap();
        std::os::unix::fs::symlink(&app_executable_path, &install_path).unwrap();

        assert_eq!(
            classify_installation(&install_path, &managed_path).unwrap(),
            EmbeddedCliState::Missing
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn installer_replaces_legacy_app_executable_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let resource_path = dir.path().join("bundled-anarlog-cli");
        let managed_path = dir.path().join(".anarlog-cli/anarlog/1.2.0");
        let app_executable_path = dir.path().join("Anarlog.app/Contents/MacOS/anarlog-cli");
        let install_path = dir.path().join("anarlog");
        std::fs::write(&resource_path, "new cli").unwrap();
        std::fs::create_dir_all(app_executable_path.parent().unwrap()).unwrap();
        std::fs::write(&app_executable_path, "old cli").unwrap();
        std::os::unix::fs::symlink(&app_executable_path, &install_path).unwrap();

        install_managed_cli(&resource_path, &managed_path, &install_path).unwrap();

        assert_eq!(std::fs::read_link(&install_path).unwrap(), managed_path);
        assert_eq!(std::fs::read_to_string(&install_path).unwrap(), "new cli");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_foreign_symlink_as_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let managed_path = dir.path().join(".anarlog-cli/anarlog/1.2.0");
        let install_path = dir.path().join("anarlog");
        std::os::unix::fs::symlink("/opt/homebrew/bin/anarlog", &install_path).unwrap();

        assert_eq!(
            classify_installation(&install_path, &managed_path).unwrap(),
            EmbeddedCliState::Conflict
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn installer_refuses_to_replace_foreign_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let resource_path = dir.path().join("bundled-anarlog-cli");
        let managed_path = dir.path().join(".anarlog-cli/anarlog/1.2.0");
        let install_path = dir.path().join("anarlog");
        let foreign_target = Path::new("/opt/homebrew/bin/anarlog");
        std::fs::write(&resource_path, "cli").unwrap();
        std::os::unix::fs::symlink(foreign_target, &install_path).unwrap();

        assert!(install_managed_cli(&resource_path, &managed_path, &install_path).is_err());
        assert_eq!(std::fs::read_link(&install_path).unwrap(), foreign_target);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn installed_cli_survives_bundled_resource_move() {
        let dir = tempfile::tempdir().unwrap();
        let resource_path = dir.path().join("Anarlog.app/Contents/MacOS/anarlog-cli");
        let install_path = dir.path().join("home/.local/bin/anarlog");
        let managed_path = managed_binary_path(&install_path, "anarlog", "1.2.0").unwrap();
        std::fs::create_dir_all(resource_path.parent().unwrap()).unwrap();
        std::fs::write(&resource_path, "cli").unwrap();
        std::fs::set_permissions(&resource_path, std::fs::Permissions::from_mode(0o644)).unwrap();

        install_managed_cli(&resource_path, &managed_path, &install_path).unwrap();
        std::fs::remove_dir_all(dir.path().join("Anarlog.app")).unwrap();

        assert_eq!(std::fs::read_to_string(&install_path).unwrap(), "cli");
        assert_ne!(
            std::fs::metadata(&install_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0
        );
        assert_eq!(
            classify_installation(&install_path, &managed_path).unwrap(),
            EmbeddedCliState::Installed
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_update_requires_installing_the_new_cli_version() {
        let dir = tempfile::tempdir().unwrap();
        let install_path = dir.path().join("home/.local/bin/anarlog");
        let old_resource_path = dir.path().join("old-cli");
        let new_resource_path = dir.path().join("new-cli");
        let old_managed_path = managed_binary_path(&install_path, "anarlog", "1.2.0").unwrap();
        let new_managed_path = managed_binary_path(&install_path, "anarlog", "1.3.0").unwrap();
        std::fs::write(&old_resource_path, "old cli").unwrap();
        std::fs::write(&new_resource_path, "new cli").unwrap();
        install_managed_cli(&old_resource_path, &old_managed_path, &install_path).unwrap();

        assert_eq!(
            classify_installation(&install_path, &new_managed_path).unwrap(),
            EmbeddedCliState::Missing
        );

        install_managed_cli(&new_resource_path, &new_managed_path, &install_path).unwrap();
        assert_eq!(std::fs::read_to_string(&install_path).unwrap(), "new cli");
        assert_eq!(
            classify_installation(&install_path, &new_managed_path).unwrap(),
            EmbeddedCliState::Installed
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_regular_file_as_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let managed_path = dir.path().join("anarlog-cli");
        let install_path = dir.path().join("anarlog");
        std::fs::write(&managed_path, "cli").unwrap();
        std::fs::write(&install_path, "other").unwrap();

        let state = classify_installation(&install_path, &managed_path).unwrap();
        assert_eq!(state, EmbeddedCliState::Conflict);
    }
}
