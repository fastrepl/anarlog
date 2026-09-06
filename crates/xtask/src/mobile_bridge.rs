use anyhow::{Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};
use xshell::{Shell, cmd};

pub(crate) fn mobile_bridge_ios() -> Result<()> {
    let sh = setup_package_shell()?;
    let ubrn = ubrn_path();
    cmd!(
        sh,
        "{ubrn} build ios --config ubrn.config.yaml --and-generate"
    )
    .run()?;
    bundle_ios_cloudsync()?;
    repair_generated_native_projects()?;
    Ok(())
}

fn bundle_ios_cloudsync() -> Result<()> {
    let sh = setup_shell()?;
    let root = crate::repo_root();
    let source = root.join("crates/cloudsync/vendor/cloudsync/apple/CloudSync.xcframework");
    let destination = root.join("packages/mobile-bridge-rn/build/CloudSync.xcframework");
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }
    cmd!(sh, "cp -R {source} {destination}").run()?;
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
    let root = crate::repo_root();
    bundle_android_cloudsync(
        &root.join("crates/cloudsync/vendor/cloudsync/android"),
        &root.join("packages/mobile-bridge-rn/android/src/main/jniLibs"),
    )?;
    repair_generated_native_projects()?;
    Ok(())
}

fn bundle_android_cloudsync(source: &Path, destination: &Path) -> Result<()> {
    for abi in ["arm64-v8a", "armeabi-v7a", "x86_64"] {
        let output = destination.join(abi);
        fs::create_dir_all(&output)?;
        fs::copy(
            source.join(abi).join("cloudsync.so"),
            output.join("libcloudsync.so"),
        )?;
    }
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
    repair_generated_native_projects()?;
    Ok(())
}

fn repair_generated_native_projects() -> Result<()> {
    let cmake_path = crate::repo_root().join("packages/mobile-bridge-rn/android/CMakeLists.txt");
    let contents = fs::read_to_string(&cmake_path)?;
    let generated = r#"execute_process(
    COMMAND node -p "require.resolve('uniffi-bindgen-react-native/package.json')"
    OUTPUT_VARIABLE UNIFFI_BINDGEN_PATH
    OUTPUT_STRIP_TRAILING_WHITESPACE
)
# Get the directory; get_filename_component and cmake_path will normalize
# paths with Windows path separators.
get_filename_component(UNIFFI_BINDGEN_PATH "${UNIFFI_BINDGEN_PATH}" DIRECTORY)"#;
    let compatible = r#"execute_process(
    COMMAND node -p "require('path').resolve(require.resolve('uniffi-bindgen-react-native'), '../../../..')"
    OUTPUT_VARIABLE UNIFFI_BINDGEN_PATH
    OUTPUT_STRIP_TRAILING_WHITESPACE
)"#;

    if contents.contains(generated) {
        fs::write(cmake_path, contents.replace(generated, compatible))?;
    }

    let podspec_path = crate::repo_root().join("packages/mobile-bridge-rn/MobileBridge.podspec");
    let contents = fs::read_to_string(&podspec_path)?;
    let generated = r#"s.vendored_frameworks = "build/MobileBridge.xcframework""#;
    let bundled = r#"s.vendored_frameworks = "build/MobileBridge.xcframework", "build/CloudSync.xcframework""#;
    if !contents.contains(bundled) {
        if !contents.contains(generated) {
            bail!(
                "generated MobileBridge podspec has no recognized vendored framework declaration"
            );
        }
        fs::write(podspec_path, contents.replace(generated, bundled))?;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packages_cloudsync_for_each_supported_android_abi() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("vendor");
        let destination = root.path().join("jniLibs");
        for abi in ["arm64-v8a", "armeabi-v7a", "x86_64"] {
            fs::create_dir_all(source.join(abi)).unwrap();
            fs::write(source.join(abi).join("cloudsync.so"), abi).unwrap();
        }

        bundle_android_cloudsync(&source, &destination).unwrap();

        for abi in ["arm64-v8a", "armeabi-v7a", "x86_64"] {
            assert_eq!(
                fs::read_to_string(destination.join(abi).join("libcloudsync.so")).unwrap(),
                abi
            );
        }
    }

    #[test]
    fn rejects_missing_android_cloudsync_binaries() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            bundle_android_cloudsync(&root.path().join("vendor"), &root.path().join("jniLibs"))
                .is_err()
        );
    }
}
