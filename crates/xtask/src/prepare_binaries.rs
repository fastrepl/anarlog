use anyhow::{Context, Result};
use std::{env, fs, process::Command};
use xshell::{Shell, cmd};

pub(crate) fn prepare_binaries() -> Result<()> {
    let root_dir = crate::repo_root();
    let src_tauri = root_dir.join("apps/desktop/src-tauri");
    let binaries_dir = src_tauri.join("binaries");

    let triple = resolve_triple()?;
    let ext = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());

    let sh = Shell::new()?;
    sh.change_dir(&src_tauri);
    cmd!(
        sh,
        "{cargo} build --release --target {triple} -p chrome-native-host"
    )
    .run()?;

    fs::create_dir_all(&binaries_dir).context("create binaries/")?;

    let src = src_tauri
        .join("target")
        .join(&triple)
        .join("release")
        .join(format!("char-chrome-native-host{ext}"));
    let dst = binaries_dir.join(format!("char-chrome-native-host-{triple}{ext}"));
    fs::copy(&src, &dst).with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;

    println!("prepare-binaries: binaries/char-chrome-native-host-{triple}{ext}");

    cmd!(
        sh,
        "{cargo} build --release --target {triple} -p cli --features desktop-macos"
    )
    .run()?;

    fs::create_dir_all(&binaries_dir).context("create binaries/")?;

    let src = src_tauri
        .join("target")
        .join(&triple)
        .join("release")
        .join(format!("char{ext}"));
    let dst = binaries_dir.join(format!("char-cli-{triple}{ext}"));
    fs::copy(&src, &dst).with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;

    println!("prepare-binaries: binaries/char-cli-{triple}{ext}");
    Ok(())
}

/// Build just the embedded `char` CLI for the Electron PoC (`apps/desktop2`).
///
/// Unlike `prepare-binaries` this skips `chrome-native-host` (not wired into
/// desktop2 yet) and drops the artifact under `apps/desktop2/binaries/` where
/// `electron-builder.config.ts#mac.extraFiles` picks it up and
/// `electron/src/paths.ts::embeddedCliPath()` resolves it in dev.
pub(crate) fn prepare_desktop2_binaries() -> Result<()> {
    let root_dir = crate::repo_root();
    let desktop2 = root_dir.join("apps/desktop2");
    let binaries_dir = desktop2.join("binaries");

    let triple = resolve_triple()?;
    let ext = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());

    let sh = Shell::new()?;
    sh.change_dir(&root_dir);
    cmd!(
        sh,
        "{cargo} build --release --target {triple} -p cli --features desktop-macos"
    )
    .run()?;

    fs::create_dir_all(&binaries_dir).context("create apps/desktop2/binaries/")?;

    // Cargo writes to the workspace target dir (not desktop2-local) — we live
    // in a single workspace, so pick it up from `<repo>/target/<triple>/release`.
    let src = root_dir
        .join("target")
        .join(&triple)
        .join("release")
        .join(format!("char{ext}"));
    let dst = binaries_dir.join(format!("char-cli-{triple}{ext}"));
    fs::copy(&src, &dst).with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;

    println!("prepare-desktop2-binaries: apps/desktop2/binaries/char-cli-{triple}{ext}");
    Ok(())
}

fn resolve_triple() -> Result<String> {
    match env::var("TAURI_ENV_TARGET_TRIPLE")
        .ok()
        .or_else(|| env::var("CARGO_BUILD_TARGET").ok())
    {
        Some(v) if !v.is_empty() => Ok(v),
        _ => rustc_host_triple(),
    }
}

fn rustc_host_triple() -> Result<String> {
    let out = Command::new("rustc")
        .arg("-vV")
        .output()
        .context("run rustc -vV")?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let host_line = stdout
        .lines()
        .find(|l| l.starts_with("host:"))
        .context("no host line in rustc -vV")?;
    let triple = host_line
        .split_whitespace()
        .nth(1)
        .context("malformed host line")?;
    Ok(triple.to_owned())
}
