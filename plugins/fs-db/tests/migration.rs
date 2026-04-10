use std::path::Path;

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

const TARGET_VERSION: &str = "1.0.2-nightly.14";

/// TEST_VAULT_DIR=<ABSOULTE_PATH> cargo test -p tauri-plugin-fs-db --test migration -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn test_migration_extraction() {
    let Some(vault_dir) = std::env::var_os("TEST_VAULT_DIR") else {
        eprintln!("TEST_VAULT_DIR not set, skipping");
        return;
    };

    let vault_dir = Path::new(&vault_dir);
    assert!(vault_dir.exists(), "TEST_VAULT_DIR does not exist");

    let temp = tempfile::tempdir().unwrap();
    let base_dir = temp.path();

    copy_dir_all(vault_dir, base_dir).unwrap();

    println!("tempdir: {}", base_dir.display());

    let target_version: hypr_version::Version = TARGET_VERSION.parse().unwrap();
    tauri_plugin_fs_db::migrations::run(base_dir, &target_version)
        .await
        .unwrap();

    assert!(base_dir.join(".char/version").exists());

    println!("done. check {} manually", base_dir.display());

    std::mem::forget(temp);
}
