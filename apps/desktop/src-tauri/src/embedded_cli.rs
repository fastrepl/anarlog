use std::path::{Path, PathBuf};

use serde::Serialize;

const DEV_BUNDLE_ID: &str = "com.char.dev";
const STABLE_BUNDLE_ID: &str = "com.char.stable";
const STAGING_BUNDLE_ID: &str = "com.char.staging";
const NIGHTLY_BUNDLE_ID: &str = "com.char.nightly";
const INSTALL_DIR: &str = "/usr/local/bin";

#[cfg_attr(target_os = "macos", allow(dead_code))]
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
    pub resource_path: Option<String>,
    pub state: EmbeddedCliState,
    pub details: Option<String>,
}

pub fn check<R: tauri::Runtime, T: tauri::Manager<R>>(manager: &T) -> EmbeddedCliStatus {
    let command_name = command_name_from_identifier(manager.config().identifier.as_ref());
    let install_path = install_path_for_command(command_name);

    #[cfg(not(target_os = "macos"))]
    {
        let _ = manager;
        return EmbeddedCliStatus {
            supported: false,
            command_name: command_name.to_string(),
            install_path: install_path.display().to_string(),
            resource_path: None,
            state: EmbeddedCliState::Unsupported,
            details: Some("Embedded CLI install is only supported on macOS.".to_string()),
        };
    }

    #[cfg(target_os = "macos")]
    {
        let Some(resource_path) = resolve_resource_path(manager) else {
            return EmbeddedCliStatus {
                supported: true,
                command_name: command_name.to_string(),
                install_path: install_path.display().to_string(),
                resource_path: None,
                state: EmbeddedCliState::ResourceMissing,
                details: Some("Embedded CLI resource is not available in this build.".to_string()),
            };
        };

        classify_status(command_name, install_path, &resource_path)
    }
}

pub fn install<R: tauri::Runtime, T: tauri::Manager<R>>(
    manager: &T,
) -> Result<EmbeddedCliStatus, String> {
    let status = check(manager);

    #[cfg(not(target_os = "macos"))]
    {
        return Ok(status);
    }

    #[cfg(target_os = "macos")]
    {
        if matches!(
            status.state,
            EmbeddedCliState::Unsupported | EmbeddedCliState::ResourceMissing
        ) {
            return Ok(status);
        }

        let resource_path = PathBuf::from(
            status
                .resource_path
                .clone()
                .ok_or_else(|| "embedded CLI resource path is missing".to_string())?,
        );
        let install_path = PathBuf::from(&status.install_path);

        install_symlink(&resource_path, &install_path)?;
        Ok(classify_status(
            &status.command_name,
            install_path,
            &resource_path,
        ))
    }
}

pub fn uninstall<R: tauri::Runtime, T: tauri::Manager<R>>(
    manager: &T,
) -> Result<EmbeddedCliStatus, String> {
    let status = check(manager);

    #[cfg(not(target_os = "macos"))]
    {
        return Ok(status);
    }

    #[cfg(target_os = "macos")]
    {
        if !matches!(status.state, EmbeddedCliState::Installed) {
            return Ok(status);
        }

        let install_path = PathBuf::from(&status.install_path);
        remove_installed_command(&install_path)?;

        Ok(check(manager))
    }
}

fn command_name_from_identifier(identifier: &str) -> &'static str {
    match identifier {
        STABLE_BUNDLE_ID => "char",
        NIGHTLY_BUNDLE_ID => "char-nightly",
        STAGING_BUNDLE_ID => "char-staging",
        DEV_BUNDLE_ID => "char-dev",
        _ => "char-dev",
    }
}

fn install_path_for_command(command_name: &str) -> PathBuf {
    Path::new(INSTALL_DIR).join(command_name)
}

#[cfg(target_os = "macos")]
fn resolve_resource_path<R: tauri::Runtime, T: tauri::Manager<R>>(manager: &T) -> Option<PathBuf> {
    use tauri::path::BaseDirectory;

    let file_name = bundled_binary_name()?;

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let bundled_path = exe_dir.join(file_name);
            if bundled_path.exists() {
                return Some(bundled_path);
            }

            let legacy_bundled_path = exe_dir.join("char-cli");
            if legacy_bundled_path.exists() {
                return Some(legacy_bundled_path);
            }
        }
    }

    if let Some(bundled_resource_path) = manager
        .path()
        .resolve(format!("cli/{file_name}"), BaseDirectory::Resource)
        .ok()
        .filter(|path| path.exists())
    {
        return Some(bundled_resource_path);
    }

    let debug_binary_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(file_name);
    if debug_binary_path.exists() {
        return Some(debug_binary_path);
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
        return Some("char-cli-aarch64-apple-darwin");
    }

    #[cfg(target_arch = "x86_64")]
    {
        return Some("char-cli-x86_64-apple-darwin");
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(target_os = "macos")]
fn classify_status(
    command_name: &str,
    install_path: PathBuf,
    resource_path: &Path,
) -> EmbeddedCliStatus {
    match classify_installation(&install_path, resource_path) {
        Ok(state) => EmbeddedCliStatus {
            supported: true,
            command_name: command_name.to_string(),
            install_path: install_path.display().to_string(),
            resource_path: Some(resource_path.display().to_string()),
            state,
            details: details_for_state(state, &install_path),
        },
        Err(error) => EmbeddedCliStatus {
            supported: true,
            command_name: command_name.to_string(),
            install_path: install_path.display().to_string(),
            resource_path: Some(resource_path.display().to_string()),
            state: EmbeddedCliState::Conflict,
            details: Some(error),
        },
    }
}

#[cfg(target_os = "macos")]
fn classify_installation(
    install_path: &Path,
    resource_path: &Path,
) -> Result<EmbeddedCliState, String> {
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(EmbeddedCliState::Missing);
        }
        Err(error) => {
            return Err(format!(
                "failed to inspect {}: {}",
                install_path.display(),
                error
            ));
        }
    };

    if !metadata.file_type().is_symlink() {
        return Ok(EmbeddedCliState::Conflict);
    }

    let installed_target = std::fs::canonicalize(install_path).map_err(|error| {
        format!(
            "failed to resolve installed command {}: {}",
            install_path.display(),
            error
        )
    })?;
    let resource_target = std::fs::canonicalize(resource_path).map_err(|error| {
        format!(
            "failed to resolve embedded CLI {}: {}",
            resource_path.display(),
            error
        )
    })?;

    if installed_target == resource_target {
        Ok(EmbeddedCliState::Installed)
    } else {
        Ok(EmbeddedCliState::Conflict)
    }
}

#[cfg(target_os = "macos")]
fn details_for_state(state: EmbeddedCliState, install_path: &Path) -> Option<String> {
    match state {
        EmbeddedCliState::Installed => Some("Command is installed and managed by this app.".into()),
        EmbeddedCliState::Missing => Some(format!(
            "Command is not installed at {}.",
            install_path.display()
        )),
        EmbeddedCliState::Conflict => Some(format!(
            "A different command already exists at {}.",
            install_path.display()
        )),
        EmbeddedCliState::Unsupported => {
            Some("Embedded CLI install is only supported on macOS.".into())
        }
        EmbeddedCliState::ResourceMissing => {
            Some("Embedded CLI resource is not available in this build.".into())
        }
    }
}

#[cfg(target_os = "macos")]
fn install_symlink(resource_path: &Path, install_path: &Path) -> Result<(), String> {
    if let Err(error) = install_symlink_direct(resource_path, install_path) {
        tracing::warn!("direct embedded CLI install failed: {}", error);
        run_privileged_shell_script(&build_install_script(resource_path, install_path))?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn install_symlink_direct(resource_path: &Path, install_path: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(
        install_path
            .parent()
            .unwrap_or_else(|| Path::new(INSTALL_DIR)),
    )?;
    remove_path_if_exists(install_path)?;
    std::os::unix::fs::symlink(resource_path, install_path)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn remove_installed_command(install_path: &Path) -> Result<(), String> {
    if let Err(error) = remove_path_if_exists(install_path) {
        tracing::warn!("direct embedded CLI uninstall failed: {}", error);
        run_privileged_shell_script(&build_uninstall_script(install_path))?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn remove_path_if_exists(path: &Path) -> Result<(), std::io::Error> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

#[cfg(target_os = "macos")]
fn build_install_script(resource_path: &Path, install_path: &Path) -> String {
    let install_dir = install_path
        .parent()
        .unwrap_or_else(|| Path::new(INSTALL_DIR));
    format!(
        "set -e; mkdir -p {install_dir}; rm -rf {install_path}; ln -s {resource_path} {install_path}",
        install_dir = shell_quote(install_dir),
        install_path = shell_quote(install_path),
        resource_path = shell_quote(resource_path),
    )
}

#[cfg(target_os = "macos")]
fn build_uninstall_script(install_path: &Path) -> String {
    format!(
        "set -e; rm -rf {install_path}",
        install_path = shell_quote(install_path),
    )
}

#[cfg(target_os = "macos")]
fn run_privileged_shell_script(script: &str) -> Result<(), String> {
    let osa = do_shell_script_with_privileges(script);
    let status = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", &osa])
        .status()
        .map_err(|error| format!("failed to launch osascript: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("administrator authorization failed".to_string())
    }
}

#[cfg(target_os = "macos")]
fn shell_quote(path: &Path) -> String {
    let escaped = path.display().to_string().replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

#[cfg(target_os = "macos")]
fn do_shell_script_with_privileges(shell_script: &str) -> String {
    let escaped = shell_script.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "do shell script \"{}\" with administrator privileges",
        escaped
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_bundle_id_to_command_name() {
        assert_eq!(command_name_from_identifier(STABLE_BUNDLE_ID), "char");
        assert_eq!(
            command_name_from_identifier(NIGHTLY_BUNDLE_ID),
            "char-nightly"
        );
        assert_eq!(
            command_name_from_identifier(STAGING_BUNDLE_ID),
            "char-staging"
        );
        assert_eq!(command_name_from_identifier(DEV_BUNDLE_ID), "char-dev");
        assert_eq!(command_name_from_identifier("unknown"), "char-dev");
    }

    #[test]
    fn install_path_uses_usr_local_bin() {
        assert_eq!(
            install_path_for_command("char"),
            PathBuf::from("/usr/local/bin/char")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classify_missing_install() {
        let dir = tempfile::tempdir().unwrap();
        let resource_path = dir.path().join("char-cli");
        std::fs::write(&resource_path, "cli").unwrap();

        let state = classify_installation(&dir.path().join("char"), &resource_path).unwrap();
        assert_eq!(state, EmbeddedCliState::Missing);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classify_installed_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let resource_path = dir.path().join("char-cli");
        std::fs::write(&resource_path, "cli").unwrap();
        let install_path = dir.path().join("char");
        std::os::unix::fs::symlink(&resource_path, &install_path).unwrap();

        let state = classify_installation(&install_path, &resource_path).unwrap();
        assert_eq!(state, EmbeddedCliState::Installed);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classify_conflict_for_different_symlink_target() {
        let dir = tempfile::tempdir().unwrap();
        let resource_path = dir.path().join("char-cli");
        let other_path = dir.path().join("other-cli");
        std::fs::write(&resource_path, "cli").unwrap();
        std::fs::write(&other_path, "cli").unwrap();
        let install_path = dir.path().join("char");
        std::os::unix::fs::symlink(&other_path, &install_path).unwrap();

        let state = classify_installation(&install_path, &resource_path).unwrap();
        assert_eq!(state, EmbeddedCliState::Conflict);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classify_conflict_for_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let resource_path = dir.path().join("char-cli");
        let install_path = dir.path().join("char");
        std::fs::write(&resource_path, "cli").unwrap();
        std::fs::write(&install_path, "other").unwrap();

        let state = classify_installation(&install_path, &resource_path).unwrap();
        assert_eq!(state, EmbeddedCliState::Conflict);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn build_install_script_quotes_paths() {
        let script = build_install_script(
            Path::new("/Applications/Char Dev.app/Contents/MacOS/char-cli"),
            Path::new("/usr/local/bin/char-dev"),
        );

        assert!(script.contains("mkdir -p '/usr/local/bin'"));
        assert!(script.contains(
            "ln -s '/Applications/Char Dev.app/Contents/MacOS/char-cli' '/usr/local/bin/char-dev'"
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn build_uninstall_script_removes_install_path() {
        let script = build_uninstall_script(Path::new("/usr/local/bin/char"));
        assert_eq!(script, "set -e; rm -rf '/usr/local/bin/char'");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn applescript_wrapper_uses_admin_privileges() {
        let script = do_shell_script_with_privileges("set -e; true");
        assert!(script.contains("with administrator privileges"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn remove_path_if_exists_handles_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let resource_path = dir.path().join("char-cli");
        let install_path = dir.path().join("char");
        std::fs::write(&resource_path, "cli").unwrap();
        std::os::unix::fs::symlink(&resource_path, &install_path).unwrap();

        remove_path_if_exists(&install_path).unwrap();
        assert!(!install_path.exists());
    }
}
