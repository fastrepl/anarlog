#[cfg(target_os = "macos")]
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(target_os = "macos")]
fn swift_runtime_rpaths() -> Vec<String> {
    let mut paths = BTreeSet::from([PathBuf::from("/usr/lib/swift")]);

    if let Some(swift_bin) = swift_bin_path() {
        if let Some(toolchain_root) = swift_bin
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
        {
            paths.insert(toolchain_root.join("lib/swift/macosx"));
        }
    }

    paths
        .into_iter()
        .filter(|path| path.exists())
        .map(|path| path.display().to_string())
        .collect()
}

#[cfg(target_os = "macos")]
fn swift_bin_path() -> Option<PathBuf> {
    let output = Command::new("xcrun")
        .args(["--find", "swift"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn main() {
    #[cfg(target_os = "macos")]
    {
        swift_rs::SwiftLinker::new("15.0")
            .with_package("permissions-swift", "./swift-permissions/")
            .link();

        for path in swift_runtime_rpaths() {
            println!("cargo:rustc-link-arg-bin=unsigned-char=-Wl,-rpath,{path}");
        }

        println!("cargo:rustc-link-lib=c++");
    }

    println!("cargo:rerun-if-changed=resources/models");
    println!("cargo:rerun-if-changed=swift-permissions/src");
    println!("cargo:rerun-if-changed=swift-permissions/Package.swift");

    tauri_build::build()
}
