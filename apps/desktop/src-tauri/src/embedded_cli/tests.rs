use super::*;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::os::unix::fs::PermissionsExt;

#[test]
fn maps_bundle_id_to_command_name() {
    assert_eq!(command_name_from_identifier(STABLE_BUNDLE_ID), "anarlog");
    assert_eq!(
        command_name_from_identifier(LEGACY_STABLE_BUNDLE_ID),
        "anarlog"
    );
    assert_eq!(command_name_from_identifier(FLATPAK_BUNDLE_ID), "anarlog");
    assert_eq!(
        command_name_from_identifier(STAGING_BUNDLE_ID),
        "anarlog-staging"
    );
    assert_eq!(command_name_from_identifier(DEV_BUNDLE_ID), "anarlog-dev");
    assert_eq!(command_name_from_identifier("unknown"), "anarlog");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn resolves_linux_x64_bundled_binary() {
    assert_eq!(
        bundled_binary_name(),
        Some("anarlog-cli-x86_64-unknown-linux-gnu")
    );
    assert_eq!(sidecar_binary_name(), "anarlog-cli");
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[test]
fn resolves_linux_arm64_bundled_binary() {
    assert_eq!(
        bundled_binary_name(),
        Some("anarlog-cli-aarch64-unknown-linux-gnu")
    );
    assert_eq!(sidecar_binary_name(), "anarlog-cli");
}

#[test]
fn finds_windows_path_entries_case_insensitively() {
    let expected = Path::new(r"C:\Users\Test\AppData\Local\Anarlog\bin");

    assert!(path_list_contains(
        r"C:\Windows;C:\USERS\TEST\APPDATA\LOCAL\ANARLOG\BIN\;C:\Tools",
        expected
    ));
    assert!(!path_list_contains(
        r"C:\Windows;C:\Users\Test\AppData\Local\Other\bin",
        expected
    ));
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn classifies_missing_install() {
    let dir = tempfile::tempdir().unwrap();
    let resource_path = dir.path().join("anarlog-cli");
    std::fs::write(&resource_path, "cli").unwrap();

    let state = classify_installation(&dir.path().join("anarlog"), &resource_path).unwrap();
    assert_eq!(state, EmbeddedCliState::Missing);
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
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
