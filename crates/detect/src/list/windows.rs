use std::collections::HashMap;

use winreg::RegKey;
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
};

use super::InstalledApp;

const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

pub fn list_installed_apps() -> Vec<InstalledApp> {
    let mut apps = HashMap::new();

    for (hive_name, hive) in [
        ("hkcu", RegKey::predef(HKEY_CURRENT_USER)),
        ("hklm", RegKey::predef(HKEY_LOCAL_MACHINE)),
    ] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            collect_uninstall_entries(&mut apps, hive_name, &hive, view);
        }
    }

    let mut apps = apps.into_values().collect::<Vec<_>>();
    apps.sort_by(|left, right| left.name.cmp(&right.name));
    apps
}

fn collect_uninstall_entries(
    apps: &mut HashMap<String, InstalledApp>,
    hive_name: &str,
    hive: &RegKey,
    view: u32,
) {
    let Ok(uninstall) = hive.open_subkey_with_flags(UNINSTALL_KEY, KEY_READ | view) else {
        return;
    };

    for key_name in uninstall.enum_keys().flatten() {
        let Ok(entry) = uninstall.open_subkey_with_flags(&key_name, KEY_READ | view) else {
            continue;
        };
        let Ok(name) = entry.get_value::<String, _>("DisplayName") else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || entry.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 {
            continue;
        }

        let dedupe_key = name.to_lowercase();
        apps.entry(dedupe_key).or_insert_with(|| InstalledApp {
            id: format!("windows:{hive_name}:{key_name}"),
            name: name.to_string(),
        });
    }
}
