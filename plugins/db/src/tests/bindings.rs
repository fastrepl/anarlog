use crate::make_specta_builder;

#[test]
fn export_types() {
    const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

    make_specta_builder::<tauri::Wry>()
        .export(
            specta_typescript::Typescript::default()
                .formatter(specta_typescript::formatter::prettier)
                .bigint(specta_typescript::BigIntExportBehavior::Number),
            OUTPUT_FILE,
        )
        .unwrap();

    let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
    std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
}

#[test]
fn default_permissions_exclude_generic_cloudsync_control() {
    let permissions = include_str!("../../permissions/default.toml");

    assert!(permissions.contains("allow-configure-cloudsync-token"));
    assert!(permissions.contains("allow-configure-e2ee-replica"));
    assert!(permissions.contains("allow-suspend-cloudsync-for-sign-out"));
    assert!(!permissions.contains("allow-begin-cloudsync-activity"));
    assert!(!permissions.contains("allow-end-cloudsync-activity"));
    assert!(!permissions.contains("\"allow-configure-cloudsync\""));
    assert!(!permissions.contains("allow-start-cloudsync"));
    assert!(!permissions.contains("allow-sync-cloudsync-now"));
}

#[test]
fn default_permissions_include_legacy_migration_workflow() {
    let permissions = include_str!("../../permissions/default.toml");

    for permission in [
        "allow-get-legacy-cleanup-status",
        "allow-get-legacy-import-report",
        "allow-cleanup-legacy-files",
        "allow-run-legacy-import",
    ] {
        assert!(permissions.contains(permission), "missing {permission}");
    }
}

#[test]
fn default_permissions_include_device_enrollment_workflow() {
    let permissions = include_str!("../../permissions/default.toml");

    for permission in [
        "allow-get-or-create-e2ee-device-identity",
        "allow-seal-e2ee-recovery-key-for-device",
        "allow-seal-workspace-e2ee-key-for-recipients",
        "allow-import-e2ee-device-enrollment",
    ] {
        assert!(permissions.contains(permission), "missing {permission}");
    }
}

#[test]
fn default_permissions_include_session_ingest() {
    let permissions = include_str!("../../permissions/default.toml");

    assert!(permissions.contains("allow-apply-session-ingest"));
}

#[test]
fn default_permissions_include_startup_ready() {
    let permissions = include_str!("../../permissions/default.toml");

    assert!(permissions.contains("allow-get-startup-status"));
    assert!(permissions.contains("allow-wait-until-ready"));
}
