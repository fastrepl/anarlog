use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use xshell::{Shell, cmd};

pub(crate) fn mobile_bridge_ios() -> Result<()> {
    let sh = setup_package_shell()?;
    let ubrn = ubrn_path();
    cmd!(
        sh,
        "{ubrn} build ios --config ubrn.config.yaml --and-generate"
    )
    .run()?;
    Ok(())
}

pub(crate) fn mobile_bridge_android() -> Result<()> {
    let sh = setup_package_shell()?;
    let ubrn = ubrn_path();
    cmd!(
        sh,
        "{ubrn} build android --config ubrn.config.yaml --and-generate"
    )
    .run()?;
    Ok(())
}

pub(crate) fn mobile_bridge_rn() -> Result<()> {
    let sh = setup_shell()?;
    let root_dir = crate::repo_root();
    let host_lib = host_library_path(&root_dir);
    let ubrn = ubrn_path();

    cmd!(sh, "cargo build -p mobile-bridge").run()?;

    if !host_lib.exists() {
        bail!("expected host library at {}", host_lib.display());
    }

    cmd!(
        sh,
        "{ubrn} generate jsi bindings --library {host_lib} --ts-dir packages/mobile-bridge-rn/src/generated --cpp-dir packages/mobile-bridge-rn/cpp/generated"
    )
    .run()?;
    let package_sh = setup_package_shell()?;
    cmd!(
        package_sh,
        "{ubrn} generate jsi turbo-module --config ubrn.config.yaml mobile_bridge"
    )
    .run()?;
    Ok(())
}

fn setup_shell() -> Result<Shell> {
    let sh = Shell::new()?;
    let root_dir = crate::repo_root();
    sh.change_dir(&root_dir);
    Ok(sh)
}

fn setup_package_shell() -> Result<Shell> {
    let sh = Shell::new()?;
    sh.change_dir(crate::repo_root().join("packages/mobile-bridge-rn"));
    Ok(sh)
}

fn host_library_path(root_dir: &Path) -> PathBuf {
    let filename = if cfg!(target_os = "macos") {
        "libmobile_bridge.dylib"
    } else if cfg!(target_os = "windows") {
        "mobile_bridge.dll"
    } else {
        "libmobile_bridge.so"
    };

    root_dir.join("target/debug").join(filename)
}

fn ubrn_path() -> PathBuf {
    let root_dir = crate::repo_root();
    let bin_name = if cfg!(target_os = "windows") {
        "ubrn.cmd"
    } else {
        "ubrn"
    };

    root_dir.join("node_modules/.bin").join(bin_name)
}
