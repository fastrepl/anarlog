use anyhow::{Context, Result};
use std::{env, fs, process::Command};
use xshell::{Shell, cmd};

pub(crate) fn prepare_binaries() -> Result<()> {
    let root_dir = crate::repo_root();
    let src_tauri = root_dir.join("apps/desktop/src-tauri");
    let binaries_dir = src_tauri.join("binaries");

    let triple = match env::var("TAURI_ENV_TARGET_TRIPLE") {
        Ok(v) => v,
        Err(_) => rustc_host_triple()?,
    };
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
        "{cargo} build --release --target {triple} -p cli --features desktop"
    )
    .run()?;

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
