use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tokio::time::{Duration, sleep};

use crate::BackgroundTask;

const MEETING_APP_LIST: [&str; 6] = [
    "zoom",
    "teams",
    "teams-for-linux",
    "webex",
    "CiscoCollabHost",
    "slack",
];

#[derive(Default)]
pub struct Detector {
    background: BackgroundTask,
}

impl crate::Observer for Detector {
    fn start(&mut self, _f: crate::DetectCallback) {
        self.background.start(|running, mut rx| async move {
            let mut detected_apps: HashSet<String> = HashSet::new();

            loop {
                tokio::select! {
                    _ = &mut rx => {
                        break;
                    }
                    _ = sleep(Duration::from_millis(500)) => {
                        if !running.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }

                        if let Ok(current_apps) = get_running_meeting_apps() {
                            for app in &current_apps {
                                if !detected_apps.contains(app) {
                                    detected_apps.insert(app.clone());
                                }
                            }

                            detected_apps.retain(|app| current_apps.contains(app));
                        }
                    }
                }
            }
        });
    }

    fn stop(&mut self) {
        self.background.stop();
    }
}

fn get_running_meeting_apps() -> Result<HashSet<String>, std::io::Error> {
    let mut apps = HashSet::new();
    let proc_dir = Path::new("/proc");

    if !proc_dir.exists() {
        return Ok(apps);
    }

    for entry in fs::read_dir(proc_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(pid_str) = path.file_name().and_then(|n| n.to_str())
            && pid_str.chars().all(|c| c.is_ascii_digit())
        {
            let comm_path = path.join("comm");
            if let Ok(comm) = fs::read_to_string(&comm_path) {
                let process_name = comm.trim();

                for &meeting_app in &MEETING_APP_LIST {
                    if process_name.to_lowercase().contains(meeting_app) {
                        apps.insert(process_name.to_string());
                        break;
                    }
                }
            }

            let cmdline_path = path.join("cmdline");
            if let Ok(cmdline) = fs::read_to_string(&cmdline_path) {
                let cmdline_lower = cmdline.to_lowercase();

                for &meeting_app in &MEETING_APP_LIST {
                    if cmdline_lower.contains(meeting_app)
                        && let Some(process_name) = cmdline.split('\0').next()
                        && let Some(basename) = process_name.split('/').next_back()
                    {
                        apps.insert(basename.to_string());
                        break;
                    }
                }
            }
        }
    }

    Ok(apps)
}
