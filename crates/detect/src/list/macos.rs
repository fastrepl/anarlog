use std::path::{Path, PathBuf};

use cidre::core_audio as ca;
use hypr_bundle::{is_app_bundle, read_bundle_info};
use objc2_app_kit::NSRunningApplication;
use sysinfo::{Pid, System};

use super::InstalledApp;

#[cfg(target_os = "macos")]
struct MicProcessSnapshot {
    pid: Option<i32>,
    is_running_input: Result<bool, crate::Error>,
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct FallbackAppMetadata {
    bundle_id: Option<String>,
    name: Option<String>,
    executable_path: Option<String>,
    executable_name: Option<String>,
}

#[cfg(target_os = "macos")]
pub fn list_installed_apps() -> Vec<InstalledApp> {
    let app_dirs = [
        "/Applications".to_string(),
        format!("{}/Applications", std::env::var("HOME").unwrap_or_default()),
    ];

    let mut apps = Vec::new();

    for dir in app_dirs {
        let path = PathBuf::from(dir);
        if !path.exists() {
            continue;
        }

        let mut stack = vec![path];
        while let Some(current) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                if is_app_bundle(&path) {
                    if let Some(info) = read_bundle_info(&path) {
                        apps.push(InstalledApp {
                            id: info.id,
                            name: info.name,
                        });
                    }
                } else {
                    stack.push(path);
                }
            }
        }
    }

    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps
}

#[cfg(not(target_os = "macos"))]
pub fn list_installed_apps() -> Vec<InstalledApp> {
    Vec::new()
}

#[cfg(target_os = "macos")]
pub fn list_mic_using_apps() -> Result<Vec<InstalledApp>, crate::Error> {
    let processes =
        ca::System::processes().map_err(|e| crate::Error::AudioProcessQuery(format!("{e:?}")))?;

    let snapshots = processes.into_iter().map(|process| {
        let pid = process.pid().ok();
        let is_running_input = process.is_running_input().map_err(|e| {
            crate::Error::AudioProcessState(match pid {
                Some(pid) => format!("pid {pid}: {e:?}"),
                None => format!("unknown pid: {e:?}"),
            })
        });

        MicProcessSnapshot {
            pid,
            is_running_input,
        }
    });

    build_mic_using_apps(snapshots, resolve_to_app, fallback_app_for_pid)
}

fn resolve_to_app(pid: i32) -> Option<InstalledApp> {
    resolve_via_nsrunningapp(pid).or_else(|| resolve_via_sysinfo(pid))
}

fn build_mic_using_apps<I, F, G>(
    processes: I,
    mut resolve_to_app: F,
    mut fallback_app_for_pid: G,
) -> Result<Vec<InstalledApp>, crate::Error>
where
    I: IntoIterator<Item = MicProcessSnapshot>,
    F: FnMut(i32) -> Option<InstalledApp>,
    G: FnMut(i32) -> InstalledApp,
{
    let mut apps = Vec::new();

    for process in processes {
        if !process.is_running_input? {
            continue;
        }

        let pid = process.pid.ok_or_else(|| {
            crate::Error::AudioProcessState("running input process missing pid".to_string())
        })?;

        apps.push(resolve_to_app(pid).unwrap_or_else(|| fallback_app_for_pid(pid)));
    }

    Ok(apps)
}

fn resolve_via_nsrunningapp(pid: i32) -> Option<InstalledApp> {
    std::panic::catch_unwind(|| resolve_via_nsrunningapp_inner(pid))
        .ok()
        .flatten()
}

fn resolve_via_nsrunningapp_inner(pid: i32) -> Option<InstalledApp> {
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;

    if let Some(bundle_url) = app.bundleURL() {
        if let Some(path_ns) = bundle_url.path() {
            let path_str = path_ns.to_string();
            if let Some(resolved) = find_outermost_app(Path::new(&path_str)) {
                return Some(resolved);
            }
        }
    }

    let bundle_id = app.bundleIdentifier()?.to_string();
    let name = app
        .localizedName()
        .map(|s| s.to_string())
        .unwrap_or_else(|| bundle_id.clone());

    Some(InstalledApp {
        id: bundle_id,
        name,
    })
}

fn resolve_via_sysinfo(pid: i32) -> Option<InstalledApp> {
    let mut sys = System::new();
    let pid = Pid::from_u32(pid as u32);
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);

    let exe_path = sys.process(pid)?.exe()?;
    find_outermost_app(exe_path)
}

fn fallback_app_for_pid(pid: i32) -> InstalledApp {
    let mut metadata = FallbackAppMetadata::default();

    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        metadata.bundle_id = app.bundleIdentifier().map(|id| id.to_string());
        metadata.name = app.localizedName().map(|name| name.to_string());
    }

    let mut sys = System::new();
    let pid_ref = Pid::from_u32(pid as u32);
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid_ref]), true);

    if let Some(process) = sys.process(pid_ref) {
        metadata.executable_path = process.exe().map(|path| path.to_string_lossy().to_string());
        let process_name = process.name().to_string_lossy().trim().to_string();
        if !process_name.is_empty() {
            metadata.executable_name = Some(process_name);
        }
    }

    build_fallback_app(pid, metadata)
}

fn build_fallback_app(pid: i32, metadata: FallbackAppMetadata) -> InstalledApp {
    if let Some(bundle_id) = metadata.bundle_id {
        return InstalledApp {
            name: metadata.name.unwrap_or_else(|| bundle_id.clone()),
            id: bundle_id,
        };
    }

    if let Some(executable_path) = metadata.executable_path {
        return InstalledApp {
            name: metadata
                .name
                .or(metadata.executable_name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| executable_path.clone()),
            id: executable_path,
        };
    }

    let pid_id = format!("pid:{pid}");
    InstalledApp {
        name: metadata.name.unwrap_or_else(|| pid_id.clone()),
        id: pid_id,
    }
}

fn find_outermost_app(path: &Path) -> Option<InstalledApp> {
    let mut outermost: Option<&Path> = None;
    let mut current = Some(path);

    while let Some(p) = current {
        if is_app_bundle(p) {
            outermost = Some(p);
        }
        current = p.parent();
    }

    outermost.and_then(|p| {
        read_bundle_info(p).map(|info| InstalledApp {
            id: info.id,
            name: info.name,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> InstalledApp {
        InstalledApp {
            id: id.to_string(),
            name: id.to_string(),
        }
    }

    #[test]
    fn test_build_mic_using_apps_errors_on_input_state_failure() {
        let snapshots = vec![MicProcessSnapshot {
            pid: Some(42),
            is_running_input: Err(crate::Error::AudioProcessState("pid 42".to_string())),
        }];

        let error = build_mic_using_apps(snapshots, |_| Some(app("resolved")), |_| app("fallback"))
            .unwrap_err();

        assert!(matches!(error, crate::Error::AudioProcessState(_)));
    }

    #[test]
    fn test_build_mic_using_apps_uses_fallback_for_unresolved_process() {
        let snapshots = vec![MicProcessSnapshot {
            pid: Some(7),
            is_running_input: Ok(true),
        }];

        let apps = build_mic_using_apps(
            snapshots,
            |_| None,
            |pid| InstalledApp {
                id: format!("pid:{pid}"),
                name: "Fallback".to_string(),
            },
        )
        .unwrap();

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "pid:7");
        assert_eq!(apps[0].name, "Fallback");
    }

    #[test]
    fn test_build_fallback_app_prefers_bundle_identity() {
        let app = build_fallback_app(
            9,
            FallbackAppMetadata {
                bundle_id: Some("us.zoom.xos".to_string()),
                name: Some("zoom.us".to_string()),
                executable_path: Some(
                    "/Applications/zoom.us.app/Contents/MacOS/zoom.us".to_string(),
                ),
                executable_name: Some("zoom.us".to_string()),
            },
        );

        assert_eq!(app.id, "us.zoom.xos");
        assert_eq!(app.name, "zoom.us");
    }

    #[test]
    fn test_build_fallback_app_uses_executable_identity_when_bundle_missing() {
        let app = build_fallback_app(
            9,
            FallbackAppMetadata {
                name: Some("Recorder".to_string()),
                executable_path: Some("/tmp/recorder".to_string()),
                executable_name: Some("recorder".to_string()),
                ..Default::default()
            },
        );

        assert_eq!(app.id, "/tmp/recorder");
        assert_eq!(app.name, "Recorder");
    }

    #[test]
    fn test_build_fallback_app_uses_pid_when_no_metadata_exists() {
        let app = build_fallback_app(9, FallbackAppMetadata::default());

        assert_eq!(app.id, "pid:9");
        assert_eq!(app.name, "pid:9");
    }

    #[test]
    #[ignore]
    fn test_list_installed_apps() {
        let apps = list_installed_apps();
        println!("Got {} apps", apps.len());
        for app in &apps {
            println!("- {} ({})", app.name, app.id);
        }
    }

    // cargo test -p detect --features list test_list_mic_using_apps -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_list_mic_using_apps() {
        let apps = list_mic_using_apps().unwrap();
        println!("Got {} apps", apps.len());
        for app in &apps {
            println!("- {} ({})", app.name, app.id);
        }
    }
}
