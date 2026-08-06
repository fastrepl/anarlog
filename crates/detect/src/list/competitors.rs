use super::InstalledApp;

const MACOS_COMPETITOR_BUNDLE_IDS: &[(&str, &str)] = &[
    ("com.granola.app", "Granola"),
    ("com.getgranola.app", "Granola"),
    ("ai.krisp.krispMac", "Krisp"),
    ("Fathom", "Fathom"),
    ("com.jamie.app", "Jamie"),
    ("ai.plaud.desktop.plaud", "Plaud"),
    ("ai.limitless.desktop", "Limitless"),
    ("com.memoryvault.MemoryVault", "Rewind"),
];

#[cfg(target_os = "windows")]
const WINDOWS_COMPETITOR_PROCESS_NAMES: &[(&str, &str)] = &[
    ("Granola.exe", "Granola"),
    ("Krisp.exe", "Krisp"),
    ("Fathom.exe", "Fathom"),
    ("Otter.exe", "Otter"),
    ("Jamie.exe", "Jamie"),
    ("Plaud.exe", "Plaud"),
    ("Limitless.exe", "Limitless"),
    ("Rewind.exe", "Rewind"),
];

#[cfg(test)]
fn competitor_name_for_macos_bundle_id(bundle_id: &str) -> Option<&'static str> {
    MACOS_COMPETITOR_BUNDLE_IDS
        .iter()
        .find_map(|(candidate, name)| (*candidate == bundle_id).then_some(*name))
}

#[cfg(target_os = "windows")]
fn competitor_name_for_windows_process(process_name: &str) -> Option<&'static str> {
    WINDOWS_COMPETITOR_PROCESS_NAMES
        .iter()
        .find_map(|(candidate, name)| {
            candidate
                .eq_ignore_ascii_case(process_name)
                .then_some(*name)
        })
}

#[cfg(target_os = "macos")]
pub fn terminate_competing_apps() -> Vec<InstalledApp> {
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::NSString;

    MACOS_COMPETITOR_BUNDLE_IDS
        .iter()
        .flat_map(|(bundle_id, name)| {
            let native_bundle_id = NSString::from_str(bundle_id);
            NSRunningApplication::runningApplicationsWithBundleIdentifier(&native_bundle_id)
                .into_iter()
                .filter_map(|application| {
                    application.forceTerminate().then(|| InstalledApp {
                        id: (*bundle_id).to_string(),
                        name: (*name).to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(target_os = "windows")]
pub fn terminate_competing_apps() -> Vec<InstalledApp> {
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    system
        .processes()
        .values()
        .filter_map(|process| {
            let process_name = process.name().to_string_lossy();
            let competitor_name = competitor_name_for_windows_process(&process_name)?;
            process.kill().then(|| InstalledApp {
                id: process_name.into_owned(),
                name: competitor_name.to_string(),
            })
        })
        .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn terminate_competing_apps() -> Vec<InstalledApp> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_allowlist_matches_only_exact_native_identifiers() {
        assert_eq!(
            competitor_name_for_macos_bundle_id("com.granola.app"),
            Some("Granola")
        );
        assert_eq!(
            competitor_name_for_macos_bundle_id("com.granola.app.helper"),
            None
        );
    }

    #[test]
    fn macos_allowlist_excludes_hosts_productivity_apps_and_self() {
        for bundle_id in [
            "us.zoom.xos",
            "com.microsoft.teams2",
            "com.cisco.webex",
            "notion.id",
            "com.tinyspeck.slackmacgap",
            "com.openai.chat",
            "com.anarlog.stable",
            "com.hyprnote.stable",
            "com.apple.FaceTime",
        ] {
            assert_eq!(competitor_name_for_macos_bundle_id(bundle_id), None);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_allowlist_excludes_hosts_productivity_apps_and_self() {
        assert_eq!(
            competitor_name_for_windows_process("Granola.exe"),
            Some("Granola")
        );
        for process_name in [
            "Zoom.exe",
            "ms-teams.exe",
            "Webex.exe",
            "Notion.exe",
            "Slack.exe",
            "ChatGPT.exe",
            "Anarlog.exe",
            "explorer.exe",
        ] {
            assert_eq!(competitor_name_for_windows_process(process_name), None);
        }
    }
}
