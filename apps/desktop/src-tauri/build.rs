#[cfg(target_os = "macos")]
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let build_rs = manifest_dir.join("build.rs");
    let swift_src = manifest_dir.join("../../../plugins/permissions/swift/check-permissions.swift");
    let binaries_dir = manifest_dir.join("binaries");
    let dst = binaries_dir.join(format!("check-permissions-{triple}"));
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let compiled = out_dir.join(format!("check-permissions-{triple}"));

    println!("cargo:rerun-if-changed={}", build_rs.display());
    println!("cargo:rerun-if-changed={}", swift_src.display());

    fs::create_dir_all(&binaries_dir).expect("create binaries/");

    if is_fresh(&dst, &[&build_rs, &swift_src]) {
        return;
    }

    let status = Command::new("swiftc")
        .args(["-O", "-o"])
        .arg(&compiled)
        .arg(&swift_src)
        // we should theoretically cross compile for the target
        // .args(["-target", &triple])
        .status()
        .expect("failed to run swiftc");

    assert!(
        status.success(),
        "swiftc failed to compile check-permissions"
    );

    if !same_contents(&compiled, &dst) {
        fs::copy(&compiled, &dst).expect("copy check-permissions binary");
    }
}

#[cfg(target_os = "macos")]
fn is_fresh(output: &Path, inputs: &[&Path]) -> bool {
    let Ok(output_modified) = fs::metadata(output).and_then(|metadata| metadata.modified()) else {
        return false;
    };

    inputs.iter().all(|input| {
        fs::metadata(input)
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified <= output_modified)
    })
}

#[cfg(target_os = "macos")]
fn same_contents(a: &Path, b: &Path) -> bool {
    match (fs::read(a), fs::read(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
