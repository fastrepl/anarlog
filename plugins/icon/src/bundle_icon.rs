use objc2_app_kit::{NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
use objc2_foundation::{NSBundle, NSString};

pub fn set(image: Option<&NSImage>) -> crate::Result<()> {
    let path = NSBundle::mainBundle().bundlePath();
    // Unbundled development and test executables must not customize their parent directory.
    if !path.to_string().ends_with(".app") {
        return Ok(());
    }
    set_for_bundle(&path, image)
}

fn set_for_bundle(path: &NSString, image: Option<&NSImage>) -> crate::Result<()> {
    // Finder/Dock uses the bundle's custom icon after the application exits.
    if NSWorkspace::sharedWorkspace().setIcon_forFile_options(
        image,
        path,
        NSWorkspaceIconCreationOptions::empty(),
    ) {
        Ok(())
    } else {
        Err(crate::Error::Custom(format!(
            "Failed to persist app icon for {path}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2::AnyThread;
    use objc2_foundation::NSData;
    use std::process::Command;

    #[test]
    fn selected_icon_survives_the_setting_process_exiting() {
        const BUNDLE_ENV: &str = "ANARLOG_ICON_TEST_BUNDLE";
        if std::env::var_os(BUNDLE_ENV).is_some() {
            let image = NSImage::initWithData(
                NSImage::alloc(),
                &NSData::with_bytes(include_bytes!(
                    "../../../apps/desktop/src-tauri/icons/src/anarlog-anagram.png"
                )),
            )
            .unwrap();
            set(Some(&image)).unwrap();
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Anarlog.app");
        let contents = bundle.join("Contents");
        let executable = contents.join("MacOS/anarlog");
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        std::fs::write(
            contents.join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>so.anarlog.icon-test</string>
<key>CFBundleExecutable</key><string>anarlog</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>"#,
        )
        .unwrap();
        let path = NSString::from_str(bundle.to_str().unwrap());
        let workspace = NSWorkspace::sharedWorkspace();
        let default_icon = workspace.iconForFile(&path).TIFFRepresentation().unwrap();

        let status = Command::new(&executable)
            .args([
                "--exact",
                "bundle_icon::tests::selected_icon_survives_the_setting_process_exiting",
            ])
            .env(BUNDLE_ENV, &bundle)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(bundle.join("Icon\r").exists());
        let saved_icon = workspace.iconForFile(&path).TIFFRepresentation().unwrap();
        assert!(!saved_icon.isEqualToData(&default_icon));

        set_for_bundle(&path, None).unwrap();
        let reset_icon = workspace.iconForFile(&path).TIFFRepresentation().unwrap();
        assert!(reset_icon.isEqualToData(&default_icon));
    }
}
