use anyhow::{Context, Result, bail};
use serde_json::Value;
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
    let endpoints = env::var("ANARLOG_UPDATER_ENDPOINTS").ok();
    let pubkey = env::var("ANARLOG_UPDATER_PUBKEY").ok();
    let release_channel = env::var("RELEASE_CHANNEL").ok();
    let updater_env = if endpoints.is_some() && pubkey.is_some() {
        None
    } else {
        match release_channel.as_deref() {
            Some(channel) => updater_env_from_tauri_conf(src_tauri, channel, triple)?,
            None => {
                println!("prepare-binaries: GPUI updater disabled (RELEASE_CHANNEL is unset)");
                None
            }
        }
    };
    let endpoints = endpoints.or_else(|| updater_env.as_ref().map(|(value, _)| value.clone()));
    let pubkey = pubkey.or_else(|| updater_env.as_ref().map(|(_, value)| value.clone()));

    let mut build = cmd!(
        sh,
        "{cargo} build --release --target {triple} -p desktop-gpui"
    );
    if let (Some(endpoints), Some(pubkey)) = (endpoints, pubkey) {
        build = build
            .env("ANARLOG_UPDATER_ENDPOINTS", endpoints)
            .env("ANARLOG_UPDATER_PUBKEY", pubkey);
    } else {
        build = build
            .env_remove("ANARLOG_UPDATER_ENDPOINTS")
            .env_remove("ANARLOG_UPDATER_PUBKEY");
    }
    build.run()?;

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

fn updater_env_from_tauri_conf(
    src_tauri: &Path,
    channel: &str,
    triple: &str,
) -> Result<Option<(String, String)>> {
    if channel.is_empty() {
        return Ok(None);
    }
    let base = read_json(&src_tauri.join("tauri.conf.json"))?;
    let pubkey = base
        .pointer("/plugins/updater/pubkey")
        .and_then(Value::as_str)
        .context("plugins.updater.pubkey is missing from tauri.conf.json")?;
    let macos_config = src_tauri.join(format!("tauri.conf.{channel}-macos.json"));
    let config_path = if triple.contains("apple") && macos_config.is_file() {
        macos_config
    } else {
        src_tauri.join(format!("tauri.conf.{channel}.json"))
    };
    let config = read_json(&config_path)?;
    let Some(updater) = config.pointer("/plugins/updater") else {
        println!(
            "prepare-binaries: GPUI updater disabled ({channel} config has no updater endpoints)"
        );
        return Ok(None);
    };
    let updater = updater
        .as_object()
        .context("plugins.updater must be an object")?;
    let Some(raw_endpoints) = updater.get("endpoints") else {
        println!(
            "prepare-binaries: GPUI updater disabled ({channel} config has no updater endpoints)"
        );
        return Ok(None);
    };
    let endpoints = raw_endpoints
        .as_array()
        .context("plugins.updater.endpoints must be an array")?
        .iter()
        .map(|endpoint| {
            endpoint
                .as_str()
                .map(str::to_owned)
                .context("plugins.updater.endpoints must contain strings")
        })
        .collect::<Result<Vec<_>>>()?;
    if endpoints.is_empty() {
        bail!("plugins.updater.endpoints is empty");
    }
    Ok(Some((endpoints.join(","), pubkey.to_owned())))
}

fn read_json(path: &Path) -> Result<Value> {
    let contents = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&contents).with_context(|| format!("parse {}", path.display()))
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
    use std::fs;

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

    #[test]
    fn derives_updater_environment_for_target_platform() {
        let root = tempfile::tempdir().unwrap();
        let src_tauri = root.path();
        fs::write(
            src_tauri.join("tauri.conf.json"),
            r#"{"plugins":{"updater":{"pubkey":"public-key"}}}"#,
        )
        .unwrap();
        fs::write(
            src_tauri.join("tauri.conf.stable.json"),
            r#"{"plugins":{"updater":{"endpoints":["linux-one","linux-two"]}}}"#,
        )
        .unwrap();
        fs::write(
            src_tauri.join("tauri.conf.stable-macos.json"),
            r#"{"plugins":{"updater":{"endpoints":["macos-one"]}}}"#,
        )
        .unwrap();

        assert_eq!(
            updater_env_from_tauri_conf(src_tauri, "stable", "x86_64-unknown-linux-gnu").unwrap(),
            Some(("linux-one,linux-two".into(), "public-key".into()))
        );
        assert_eq!(
            updater_env_from_tauri_conf(src_tauri, "stable", "aarch64-apple-darwin").unwrap(),
            Some(("macos-one".into(), "public-key".into()))
        );
    }

    #[test]
    fn treats_channel_without_updater_endpoints_as_disabled() {
        let root = tempfile::tempdir().unwrap();
        let src_tauri = root.path();
        fs::write(
            src_tauri.join("tauri.conf.json"),
            r#"{"plugins":{"updater":{"pubkey":"public-key"}}}"#,
        )
        .unwrap();
        fs::write(
            src_tauri.join("tauri.conf.staging.json"),
            r#"{"plugins":{}}"#,
        )
        .unwrap();

        assert_eq!(
            updater_env_from_tauri_conf(src_tauri, "staging", "x86_64-unknown-linux-gnu").unwrap(),
            None
        );
    }
}
