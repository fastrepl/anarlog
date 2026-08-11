use objc2::rc::Retained;
use objc2::{AnyThread, Message};
use objc2_app_kit::{NSImage, NSWorkspace};
use objc2_foundation::{NSBundle, NSSize, NSString, NSURL};
use std::path::{Path, PathBuf};

use crate::{Error, Result};

#[derive(Debug)]
pub(crate) struct HostApp {
    pub(crate) display_name: String,
    pub(crate) bundle_url: Retained<NSURL>,
    pub(crate) icon: Retained<NSImage>,
}

impl HostApp {
    pub(crate) fn current() -> Result<Self> {
        let bundle = NSBundle::mainBundle();
        let bundle_url = bundle.bundleURL();
        Self::from_bundle_url(&bundle, &bundle_url)
    }

    fn from_bundle_url(bundle: &NSBundle, bundle_url: &NSURL) -> Result<Self> {
        let display_name = bundle
            .objectForInfoDictionaryKey(&NSString::from_str("CFBundleDisplayName"))
            .or_else(|| bundle.objectForInfoDictionaryKey(&NSString::from_str("CFBundleName")))
            .and_then(|value| value.downcast_ref::<NSString>().map(ToString::to_string))
            .unwrap_or_else(|| bundle_filename(bundle_url));

        let path = bundle_url
            .path()
            .ok_or_else(|| Error::Assistant("failed to resolve app bundle path".to_string()))?;
        let icon = bundle_icon(bundle).unwrap_or_else(|| {
            let workspace = NSWorkspace::sharedWorkspace();
            workspace.iconForFile(&path)
        });
        icon.setSize(NSSize::new(48.0, 48.0));

        Ok(Self {
            display_name,
            bundle_url: bundle_url.retain(),
            icon,
        })
    }
}

fn bundle_icon(bundle: &NSBundle) -> Option<Retained<NSImage>> {
    for icon_name in bundle_icon_names(bundle) {
        if let Some(icon) = load_bundle_icon(bundle, &icon_name) {
            return Some(icon);
        }
    }
    None
}

fn bundle_icon_names(bundle: &NSBundle) -> Vec<String> {
    let mut names = Vec::new();

    for key in ["CFBundleIconFile", "CFBundleIconName"] {
        let Some(name) = bundle
            .objectForInfoDictionaryKey(&NSString::from_str(key))
            .and_then(|value| value.downcast_ref::<NSString>().map(ToString::to_string))
        else {
            continue;
        };
        push_unique(&mut names, name);
    }

    // Alternate app icons ship as `AppIcon.icns` resources and can replace the
    // Info.plist icon at runtime, so fall back to the bundled resources rather
    // than trusting `CFBundleIconFile` alone.
    push_unique(&mut names, "icon.icns".to_string());
    push_unique(&mut names, "AppIcon.icns".to_string());
    push_unique(&mut names, "AppIcon".to_string());

    names
}

fn push_unique(values: &mut Vec<String>, value: String) {
    let value = value.trim();
    if value.is_empty() || values.iter().any(|existing| existing == value) {
        return;
    }
    values.push(value.to_string());
}

fn load_bundle_icon(bundle: &NSBundle, icon_name: &str) -> Option<Retained<NSImage>> {
    let resources_path = bundle.resourcePath()?.to_string();
    for path in icon_candidate_paths(&resources_path, icon_name) {
        if !path.exists() {
            continue;
        }
        let path = NSString::from_str(&path.to_string_lossy());
        if let Some(image) = NSImage::initWithContentsOfFile(NSImage::alloc(), &path) {
            return Some(image);
        }
    }
    None
}

fn icon_candidate_paths(resources_path: &str, icon_name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let icon_path = Path::new(icon_name);
    if icon_path.extension().is_some() {
        paths.push(Path::new(resources_path).join(icon_path));
    } else {
        paths.push(Path::new(resources_path).join(format!("{icon_name}.icns")));
        paths.push(Path::new(resources_path).join(icon_path));
    }
    paths
}

fn bundle_filename(bundle_url: &NSURL) -> String {
    bundle_url
        .lastPathComponent()
        .map(|component| component.to_string())
        .and_then(|component| {
            component
                .strip_suffix(".app")
                .map(str::to_string)
                .or(Some(component))
        })
        .unwrap_or_else(|| "App".to_string())
}
