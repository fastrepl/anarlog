fn main() {
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-fapple-link-rtlib");

    #[cfg(target_os = "macos")]
    build_check_permissions();

    tauri_build::build()
}

#[cfg(target_os = "macos")]
fn build_check_permissions() {
    let triple = std::env::var("TARGET").unwrap();

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let swift_src = manifest_dir.join("../../../plugins/permissions/swift/check-permissions.swift");
    let binaries_dir = manifest_dir.join("binaries");
    let dst = binaries_dir.join(format!("check-permissions-{triple}"));

    println!("cargo:rerun-if-changed={}", swift_src.display());

    std::fs::create_dir_all(&binaries_dir).expect("create binaries/");

    let status = std::process::Command::new("swiftc")
        .args(["-O", "-o"])
        .arg(&dst)
        .arg(&swift_src)
        // we should theoretically cross compile for the target
        // .args(["-target", &triple])
        .status()
        .expect("failed to run swiftc");

    assert!(
        status.success(),
        "swiftc failed to compile check-permissions"
    );
}
