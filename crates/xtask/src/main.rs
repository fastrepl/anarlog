use anyhow::{Context, Result, bail};
use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use xshell::{Shell, cmd};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }

    match args.first().map(String::as_str) {
        Some("prepare-binaries") => prepare_binaries(),
        Some("mobile-bridge") => match args.get(1).map(String::as_str) {
            None | Some("ios") => mobile_bridge_ios(),
            Some(arg) => bail!("unknown mobile-bridge target: {arg}"),
        },
        None => {
            print_help();
            Ok(())
        }
        Some(cmd) => bail!("unknown xtask command: {cmd}"),
    }
}

fn print_help() {
    println!(
        "xtask\n\nUSAGE:\n    cargo xtask prepare-binaries\n    cargo xtask mobile-bridge [ios]\n",
    );
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .expect("xtask crate lives under crates/")
}

fn prepare_binaries() -> Result<()> {
    let root_dir = repo_root();
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

fn mobile_bridge_ios() -> Result<()> {
    let sh = Shell::new()?;
    let root_dir = repo_root();
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
