use std::collections::HashMap;

use winreg::RegKey;
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
};

use super::InstalledApp;

const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

struct ComGuard;

impl ComGuard {
    fn initialize() -> Result<Self, crate::Error> {
        wasapi::initialize_mta().ok().map_err(|error| {
            crate::Error::AudioProcessQuery(format!("COM initialization failed: {error}"))
        })?;
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        wasapi::deinitialize();
    }
}

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

pub fn list_mic_using_apps() -> Result<Vec<InstalledApp>, crate::Error> {
    let _com = ComGuard::initialize()?;
    let enumerator = wasapi::DeviceEnumerator::new()
        .map_err(|error| crate::Error::AudioProcessQuery(error.to_string()))?;
    let devices = enumerator
        .get_device_collection(&wasapi::Direction::Capture)
        .map_err(|error| crate::Error::AudioProcessQuery(error.to_string()))?;

    let mut process_ids = Vec::new();
    for device in &devices {
        let Ok(device) = device else {
            continue;
        };
        let Ok(manager) = device.get_iaudiosessionmanager() else {
            continue;
        };
        let Ok(sessions) = manager.get_audiosessionenumerator() else {
            continue;
        };
        let Ok(count) = sessions.get_count() else {
            continue;
        };

        for index in 0..count {
            let Ok(session) = sessions.get_session(index) else {
                continue;
            };
            if !matches!(session.get_state(), Ok(wasapi::SessionState::Active)) {
                continue;
            }
            let Ok(process_id) = session.get_process_id() else {
                continue;
            };
            if process_id != 0 {
                process_ids.push(process_id);
            }
        }
    }

    process_ids.sort_unstable();
    process_ids.dedup();

    let mut system = sysinfo::System::new();
    let sysinfo_process_ids = process_ids
        .iter()
        .copied()
        .map(sysinfo::Pid::from_u32)
        .collect::<Vec<_>>();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&sysinfo_process_ids), true);

    let mut apps = process_ids
        .into_iter()
        .filter_map(|process_id| app_for_process(&system, process_id))
        .fold(HashMap::<String, InstalledApp>::new(), |mut apps, app| {
            apps.entry(app.id.clone()).or_insert(app);
            apps
        })
        .into_values()
        .collect::<Vec<_>>();
    apps.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(apps)
}

fn app_for_process(system: &sysinfo::System, process_id: u32) -> Option<InstalledApp> {
    let process = system.process(sysinfo::Pid::from_u32(process_id))?;
    let process_name = process.name().to_string_lossy();
    let name = process_name.trim().trim_end_matches(".exe").trim();
    if name.is_empty() {
        return None;
    }

    Some(InstalledApp {
        id: name.to_ascii_lowercase(),
        name: name.to_string(),
    })
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
