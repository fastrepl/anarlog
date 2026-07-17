#[cfg(target_os = "macos")]
use std::{
    collections::BTreeSet,
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

#[cfg(target_os = "macos")]
const SONIQO_SWIFT_MACOS_DEPLOYMENT_TARGET: &str = "14.0";
#[cfg(target_os = "macos")]
const SONIQO_METALLIB_MACOS_DEPLOYMENT_TARGET: &str = "15.0";

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
fn target_is_macos_apple_silicon() -> bool {
    env::var("CARGO_CFG_TARGET_OS").is_ok_and(|value| value == "macos")
        && env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|value| value == "aarch64")
}

#[cfg(target_os = "macos")]
fn cargo_profile() -> &'static str {
    match env::var("PROFILE").as_deref() {
        Ok("release") => "release",
        _ => "debug",
    }
}

#[cfg(target_os = "macos")]
fn profile_target_dir(out_dir: &Path) -> Option<PathBuf> {
    out_dir.ancestors().nth(3).map(Path::to_path_buf)
}

#[cfg(target_os = "macos")]
fn find_mlx_metallib(root: &Path, profile: &str) -> Option<PathBuf> {
    [
        root.join(profile).join("mlx.metallib"),
        root.join("arm64-apple-macosx")
            .join(profile)
            .join("mlx.metallib"),
    ]
    .into_iter()
    .find(|path| path.exists())
}

#[cfg(target_os = "macos")]
fn swift_profile_dir(root: &Path, profile: &str) -> PathBuf {
    let direct = root.join(profile);
    if direct.exists() {
        return direct;
    }

    root.join("arm64-apple-macosx").join(profile)
}

#[cfg(target_os = "macos")]
fn collect_files(root: &Path, predicate: impl Fn(&Path) -> bool) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if predicate(&path) {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}

#[cfg(target_os = "macos")]
fn newest_modified_time(files: &[PathBuf]) -> io::Result<SystemTime> {
    files
        .iter()
        .map(|path| fs::metadata(path)?.modified())
        .try_fold(SystemTime::UNIX_EPOCH, |newest, modified| {
            modified.map(|modified| newest.max(modified))
        })
}

#[cfg(target_os = "macos")]
fn metallib_is_fresh(metallib: &Path, inputs: &[PathBuf]) -> bool {
    let Ok(output_modified) = fs::metadata(metallib).and_then(|metadata| metadata.modified())
    else {
        return false;
    };
    let Ok(input_modified) = newest_modified_time(inputs) else {
        return false;
    };

    output_modified >= input_modified
}

#[cfg(target_os = "macos")]
fn run_command(mut command: Command, context: &str) {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to run {context}: {error}"));

    if !output.status.success() {
        panic!(
            "{context} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(target_os = "macos")]
fn prepare_swift_package(swift_build_dir: &Path) {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"),
    );
    let swift_package_dir = manifest_dir.join("swift-lib");

    let mut command = Command::new("swift");
    command
        .arg("package")
        .arg("--package-path")
        .arg(&swift_package_dir)
        .arg("--scratch-path")
        .arg(swift_build_dir)
        .arg("resolve");
    run_command(command, "resolving Soniqo Swift dependencies");

    patch_speech_swift_manifest(swift_build_dir);
}

#[cfg(target_os = "macos")]
fn patch_speech_swift_manifest(swift_build_dir: &Path) {
    let manifest = swift_build_dir
        .join("checkouts")
        .join("speech-swift")
        .join("Package.swift");
    let target = format!(".macOS(\"{SONIQO_SWIFT_MACOS_DEPLOYMENT_TARGET}\")");
    let contents = fs::read_to_string(&manifest).unwrap_or_else(|error| {
        panic!(
            "failed to read Soniqo speech-swift manifest {}: {error}",
            manifest.display()
        )
    });

    if contents.contains(&target) {
        return;
    }

    let patched = contents.replace(".macOS(\"15.0\")", &target);
    if patched == contents {
        panic!(
            "failed to patch Soniqo speech-swift manifest {}; expected macOS 15.0 platform declaration",
            manifest.display()
        );
    }

    let mut permissions = fs::metadata(&manifest)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read permissions for Soniqo speech-swift manifest {}: {error}",
                manifest.display()
            )
        })
        .permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(&manifest, permissions).unwrap_or_else(|error| {
            panic!(
                "failed to make Soniqo speech-swift manifest writable {}: {error}",
                manifest.display()
            )
        });
    }

    fs::write(&manifest, patched).unwrap_or_else(|error| {
        panic!(
            "failed to patch Soniqo speech-swift manifest {}: {error}",
            manifest.display()
        )
    });
}

#[cfg(target_os = "macos")]
fn compile_mlx_metallib(swift_build_dir: &Path, profile: &str) -> PathBuf {
    let mlx_swift_dir = swift_build_dir.join("checkouts").join("mlx-swift");
    let kernels_dir = mlx_swift_dir
        .join("Source")
        .join("Cmlx")
        .join("mlx")
        .join("mlx")
        .join("backend")
        .join("metal")
        .join("kernels");
    let output_dir = swift_profile_dir(swift_build_dir, profile);
    let output_metallib = output_dir.join("mlx.metallib");

    let metal_sources = collect_files(&kernels_dir, |path| {
        path.extension().is_some_and(|ext| ext == "metal")
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_nax.metal"))
    })
    .unwrap_or_else(|error| {
        panic!(
            "failed to collect Soniqo MLX Metal sources from {}: {error}",
            kernels_dir.display()
        )
    });

    if metal_sources.is_empty() {
        panic!(
            "no Soniqo MLX Metal sources found under {}",
            kernels_dir.display()
        );
    }

    let metal_headers = collect_files(&kernels_dir, |path| {
        path.extension().is_some_and(|ext| ext == "h")
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_nax.h"))
    })
    .unwrap_or_else(|error| {
        panic!(
            "failed to collect Soniqo MLX Metal headers from {}: {error}",
            kernels_dir.display()
        )
    });
    let inputs = metal_sources
        .iter()
        .chain(metal_headers.iter())
        .cloned()
        .collect::<Vec<_>>();

    if metallib_is_fresh(&output_metallib, &inputs) {
        return output_metallib;
    }

    fs::create_dir_all(&output_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create Soniqo MLX metallib output directory {}: {error}",
            output_dir.display()
        )
    });

    let air_dir = output_dir.join("mlx-metallib-air");
    if air_dir.exists() {
        fs::remove_dir_all(&air_dir).unwrap_or_else(|error| {
            panic!(
                "failed to clean Soniqo MLX Metal temp directory {}: {error}",
                air_dir.display()
            )
        });
    }
    fs::create_dir_all(&air_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create Soniqo MLX Metal temp directory {}: {error}",
            air_dir.display()
        )
    });

    let include_root = mlx_swift_dir.join("Source").join("Cmlx").join("mlx");
    let mut air_files = Vec::with_capacity(metal_sources.len());

    for (index, source) in metal_sources.iter().enumerate() {
        let air_file = air_dir.join(format!("{index}.air"));
        let mut command = Command::new("xcrun");
        command
            .args([
                "-sdk",
                "macosx",
                "metal",
                "-x",
                "metal",
                "-std=metal3.2",
                "-Wall",
                "-Wextra",
                "-fno-fast-math",
                "-Wno-c++17-extensions",
                "-Wno-c++20-extensions",
                "-c",
            ])
            .arg(format!(
                "-mmacosx-version-min={SONIQO_METALLIB_MACOS_DEPLOYMENT_TARGET}"
            ))
            .arg(source)
            .arg(format!("-I{}", kernels_dir.display()))
            .arg(format!("-I{}", include_root.display()))
            .arg("-o")
            .arg(&air_file);
        run_command(command, &format!("compiling {}", source.display()));
        air_files.push(air_file);
    }

    let mut command = Command::new("xcrun");
    command.args(["-sdk", "macosx", "metallib"]);
    command.args(&air_files);
    command.arg("-o").arg(&output_metallib);
    run_command(command, "linking Soniqo MLX metallib");

    output_metallib
}

#[cfg(target_os = "macos")]
fn copy_if_changed(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() && fs::read(source)? == fs::read(destination)? {
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination)?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn desktop_resource_metallib_path() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    Some(
        manifest_dir
            .parent()?
            .parent()?
            .join("apps/desktop/src-tauri/resources/mlx.metallib"),
    )
}

#[cfg(target_os = "macos")]
fn build_mlx_metallib() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let swift_build_dir = out_dir.join("swift-rs").join("soniqo-swift");
    let profile = cargo_profile();
    let built_metallib = compile_mlx_metallib(&swift_build_dir, profile);
    let metallib = find_mlx_metallib(&swift_build_dir, profile).unwrap_or_else(|| {
        panic!(
            "Soniqo MLX metallib build completed but mlx.metallib was not found under {}",
            swift_build_dir.display()
        )
    });
    assert_eq!(built_metallib, metallib);

    if let Some(target_dir) = profile_target_dir(&out_dir) {
        let destination = target_dir.join("mlx.metallib");
        copy_if_changed(&metallib, &destination).unwrap_or_else(|error| {
            panic!(
                "failed to copy Soniqo MLX metallib to {}: {error}",
                destination.display()
            )
        });
    }

    if let Some(destination) = desktop_resource_metallib_path() {
        if destination.parent().is_some_and(Path::exists) {
            copy_if_changed(&metallib, &destination).unwrap_or_else(|error| {
                panic!(
                    "failed to copy Soniqo MLX metallib to {}: {error}",
                    destination.display()
                )
            });
        }
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    {
        if !target_is_macos_apple_silicon() {
            println!(
                "cargo:warning=Soniqo speech-swift linking is only available on macOS Apple Silicon"
            );
            return;
        }

        let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
        let swift_build_dir = out_dir.join("swift-rs").join("soniqo-swift");
        prepare_swift_package(&swift_build_dir);

        swift_rs::SwiftLinker::new(SONIQO_SWIFT_MACOS_DEPLOYMENT_TARGET)
            .with_package("soniqo-swift", "./swift-lib/")
            .link();

        build_mlx_metallib();

        for path in swift_runtime_rpaths() {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{path}");
        }

        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rerun-if-changed=swift-lib/src");
        println!("cargo:rerun-if-changed=swift-lib/Package.swift");
        println!("cargo:rerun-if-changed=swift-lib/Package.resolved");
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!(
            "cargo:warning=Soniqo speech-swift linking is only available on macOS Apple Silicon"
        );
    }
}
