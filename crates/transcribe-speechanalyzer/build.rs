#[cfg(target_os = "macos")]
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(target_os = "macos")]
const SWIFT_MACOS_DEPLOYMENT_TARGET: &str = "14.2";

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

#[cfg(target_os = "macos")]
fn swift_runtime_rpaths() -> Vec<String> {
    let mut paths = BTreeSet::from([PathBuf::from("/usr/lib/swift")]);

    if let Some(swift_bin) = swift_bin_path()
        && let Some(toolchain_root) = swift_bin
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
    {
        paths.insert(toolchain_root.join("lib/swift/macosx"));
    }

    paths
        .into_iter()
        .filter(|path| path.exists())
        .map(|path| path.display().to_string())
        .collect()
}

fn main() {
    #[cfg(target_os = "macos")]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
            println!("cargo:warning=Apple Speech linking is only available on macOS");
            return;
        }

        swift_rs::SwiftLinker::new(SWIFT_MACOS_DEPLOYMENT_TARGET)
            .with_package("apple-speech-swift", "./swift-lib/")
            .link();

        for path in swift_runtime_rpaths() {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{path}");
        }

        println!("cargo:rerun-if-changed=swift-lib/src");
        println!("cargo:rerun-if-changed=swift-lib/Package.swift");
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!("cargo:warning=Apple Speech linking is only available on macOS");
    }
}
