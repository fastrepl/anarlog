use anyhow::{Context, Result, bail};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};
use xshell::{Shell, cmd};

pub(crate) fn mobile_bridge_ios() -> Result<()> {
    let sh = Shell::new()?;
    let root_dir = crate::repo_root();
    sh.change_dir(&root_dir);

    let out_dir = root_dir.join("apps/mobile");
    let generated_dir = out_dir.join("ios/HyprMobile/Generated");
    let xcframework_dir = out_dir.join("ios/HyprMobile/MobileBridge.xcframework");

    if generated_dir.exists() {
        fs::remove_dir_all(&generated_dir).context("remove Generated/")?;
    }
    fs::create_dir_all(&generated_dir).context("create Generated/")?;

    if xcframework_dir.exists() {
        fs::remove_dir_all(&xcframework_dir).context("remove existing xcframework")?;
    }

    cmd!(sh, "cargo build -p mobile-bridge --release").run()?;

    let host_lib = root_dir.join("target/release/libmobile_bridge.dylib");
    if !host_lib.exists() {
        bail!("expected host library at {}", host_lib.display());
    }

    cmd!(
        sh,
        "cargo build -p mobile-bridge --target aarch64-apple-ios --release"
    )
    .run()?;
    cmd!(
        sh,
        "cargo build -p mobile-bridge --target aarch64-apple-ios-sim --release"
    )
    .run()?;

    cmd!(
        sh,
        "cargo run -p uniffi-bindgen --bin uniffi-bindgen -- generate --library {host_lib} --language swift --out-dir {generated_dir}"
    )
    .run()?;

    let ffi_header = find_single_file(&generated_dir, |p| {
        p.file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|n| n.ends_with("FFI.h"))
    })
    .context("locate generated FFI header")?;
    let ffi_module_name = ffi_header
        .file_stem()
        .and_then(OsStr::to_str)
        .context("ffi header stem")?
        .to_owned();
    let ffi_header_name = ffi_header
        .file_name()
        .and_then(OsStr::to_str)
        .context("ffi header filename")?
        .to_owned();

    let device_lib = root_dir.join("target/aarch64-apple-ios/release/libmobile_bridge.a");
    let sim_lib = root_dir.join("target/aarch64-apple-ios-sim/release/libmobile_bridge.a");
    if !device_lib.exists() {
        bail!("expected iOS device library at {}", device_lib.display());
    }
    if !sim_lib.exists() {
        bail!("expected iOS simulator library at {}", sim_lib.display());
    }

    let device_headers = tempfile::tempdir().context("create device headers dir")?;
    let sim_headers = tempfile::tempdir().context("create sim headers dir")?;

    fs::copy(
        generated_dir.join(&ffi_header_name),
        device_headers.path().join(&ffi_header_name),
    )
    .context("copy device header")?;
    fs::copy(
        generated_dir.join(&ffi_header_name),
        sim_headers.path().join(&ffi_header_name),
    )
    .context("copy sim header")?;

    let modulemap = format!(
        "module {ffi_module_name} {{\n    header \"{ffi_header_name}\"\n    export *\n}}\n",
    );
    fs::write(
        device_headers.path().join("module.modulemap"),
        modulemap.as_bytes(),
    )
    .context("write device modulemap")?;
    fs::write(
        sim_headers.path().join("module.modulemap"),
        modulemap.as_bytes(),
    )
    .context("write sim modulemap")?;

    let device_headers_dir = device_headers.path();
    let sim_headers_dir = sim_headers.path();

    cmd!(
        sh,
        "xcodebuild -create-xcframework -library {device_lib} -headers {device_headers_dir} -library {sim_lib} -headers {sim_headers_dir} -output {xcframework_dir}"
    )
    .run()?;

    Ok(())
}

fn find_single_file(dir: &Path, mut predicate: impl FnMut(&Path) -> bool) -> Result<PathBuf> {
    let mut matches = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
        let entry = entry.context("read dir entry")?;
        let path = entry.path();
        if path.is_file() && predicate(&path) {
            matches.push(path);
        }
    }

    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => bail!("no matching files found in {}", dir.display()),
        n => bail!("expected 1 matching file in {}, found {n}", dir.display()),
    }
}
