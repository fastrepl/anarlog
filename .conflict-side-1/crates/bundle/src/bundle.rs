use crate::proc::*;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct BundleInfo {
    pub id: String,
    pub name: String,
}

pub fn is_app_bundle(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()) == Some("app")
}

pub fn read_bundle_info(app_path: &Path) -> Option<BundleInfo> {
    let plist_path = app_path.join("Contents/Info.plist");
    let plist_data = std::fs::read(&plist_path).ok()?;
    let plist: plist::Dictionary = plist::from_bytes(&plist_data).ok()?;

    let id = plist
        .get("CFBundleIdentifier")
        .and_then(|v| v.as_string())?
        .to_string();

    let name = plist
        .get("CFBundleDisplayName")
        .or_else(|| plist.get("CFBundleName"))
        .and_then(|v| v.as_string())?
        .to_string();

    Some(BundleInfo { id, name })
}

#[cfg(all(feature = "cidre", not(feature = "objc2")))]
pub fn bundle_id_for_pid(pid: i32) -> Option<String> {
    let running_app = cidre::ns::RunningApp::with_pid(pid)?;
    Some(running_app.bundle_id()?.to_string())
}

#[cfg(all(feature = "objc2", not(feature = "cidre")))]
pub fn bundle_id_for_pid(pid: i32) -> Option<String> {
    use objc2_app_kit::NSRunningApplication;

    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    let bundle_id = app.bundleIdentifier()?;
    Some(bundle_id.to_string())
}

pub fn get_ancestor_bundle_id() -> Option<String> {
    let mut pid = std::process::id();

    for _ in 0..32 {
        if let Some(bundle_id) = bundle_id_for_pid(pid as i32) {
            return Some(bundle_id);
        }

        let parent = parent_pid_for_pid(pid)?;
        if parent <= 1 || parent == pid {
            return None;
        }
        pid = parent;
    }

    None
}
