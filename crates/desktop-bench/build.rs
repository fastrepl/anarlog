use std::path::Path;

// Artifacts record tool versions; Cargo does not expose dependency versions to
// the crate, so read the pinned sysinfo version from the workspace lockfile.
fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let lockfile = Path::new(&manifest_dir).join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lockfile.display());

    let version = std::fs::read_to_string(&lockfile)
        .ok()
        .and_then(|lock| locked_version(&lock, "sysinfo"))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=SYSINFO_VERSION={version}");
}

fn locked_version(lockfile: &str, package: &str) -> Option<String> {
    let mut lines = lockfile.lines();
    while let Some(line) = lines.next() {
        if line.trim() == format!("name = \"{package}\"") {
            return lines
                .next()?
                .trim()
                .strip_prefix("version = \"")?
                .strip_suffix('"')
                .map(str::to_string);
        }
    }
    None
}
