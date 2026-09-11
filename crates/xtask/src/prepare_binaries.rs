use anyhow::{Context, Result};
use std::{env, ffi::OsStr, fs, path::Path, process::Command};
use xshell::{Shell, cmd};

pub(crate) fn prepare_binaries() -> Result<()> {
    let root_dir = crate::repo_root();
    let src_tauri = root_dir.join("apps/desktop/src-tauri");
    let binaries_dir = src_tauri.join("binaries");
    let embedded_cli_dir = src_tauri.join("resources").join("cli");

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
        "{cargo} build --release --target {triple} -p anarlog-cli"
    )
    .run()?;

    fs::create_dir_all(&embedded_cli_dir).context("create resources/cli/")?;

    let src = src_tauri
        .join("target")
        .join(&triple)
        .join("release")
        .join(format!("anarlog{ext}"));
    let dst = embedded_cli_dir.join(format!("anarlog-cli-{triple}{ext}"));
    fs::copy(&src, &dst).with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;

    println!("prepare-binaries: resources/cli/anarlog-cli-{triple}{ext}");

    // Opt-in: "1" requires the GPUI sidecar; "optional" skips it on build or copy failure.
    match gpui_sidecar_mode(env::var_os("ANARLOG_GPUI_SIDECAR").as_deref()) {
        GpuiSidecar::Required => {
            build_gpui_sidecar(&sh, &cargo, &triple, ext, &src_tauri, &binaries_dir)?;
        }
        GpuiSidecar::Optional => {
            if let Err(error) =
                build_gpui_sidecar(&sh, &cargo, &triple, ext, &src_tauri, &binaries_dir)
            {
                eprintln!(
                    "prepare-binaries: skipping binaries/anarlog-gpui-{triple}{ext} \
                     (optional build failed: {error})"
                );
            }
        }
        GpuiSidecar::Off => {}
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
enum GpuiSidecar {
    Off,
    Required,
    Optional,
}

fn gpui_sidecar_mode(value: Option<&OsStr>) -> GpuiSidecar {
    match value.and_then(OsStr::to_str) {
        Some("1") => GpuiSidecar::Required,
        Some("optional") => GpuiSidecar::Optional,
        _ => GpuiSidecar::Off,
    }
}

fn build_gpui_sidecar(
    sh: &Shell,
    cargo: &str,
    triple: &str,
    ext: &str,
    src_tauri: &Path,
    binaries_dir: &Path,
) -> Result<()> {
    cmd!(
        sh,
        "{cargo} build --release --target {triple} -p desktop-gpui"
    )
    .run()?;

    let src = src_tauri
        .join("target")
        .join(triple)
        .join("release")
        .join(format!("anarlog-gpui{ext}"));
    let dst = binaries_dir.join(format!("anarlog-gpui-{triple}{ext}"));
    fs::copy(&src, &dst).with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;

    println!("prepare-binaries: binaries/anarlog-gpui-{triple}{ext}");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gpui_sidecar_modes() {
        assert_eq!(gpui_sidecar_mode(None), GpuiSidecar::Off);
        assert_eq!(
            gpui_sidecar_mode(Some(OsStr::new("1"))),
            GpuiSidecar::Required
        );
        assert_eq!(
            gpui_sidecar_mode(Some(OsStr::new("optional"))),
            GpuiSidecar::Optional
        );
        assert_eq!(
            gpui_sidecar_mode(Some(OsStr::new("true"))),
            GpuiSidecar::Off
        );
    }
}
