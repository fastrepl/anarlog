use std::fs::TryLockError;
use std::path::Path;
use std::sync::{Arc, Mutex};

const LAUNCH_LOCK_FILENAME: &str = "launch.lock";
const SLOW_STARTUP_INDICATOR_DELAY: std::time::Duration = std::time::Duration::from_secs(3);

// Startup migrations can hold the database for minutes before any window or
// plugin exists, so single-instance semantics are enforced with an OS file
// lock that is held from process start until the single-instance plugin is
// initialized. The OS releases it automatically if the process dies.
pub struct LaunchLock {
    _file: std::fs::File,
}

pub enum LaunchLockState {
    Acquired(LaunchLock),
    HeldByAnotherProcess,
    Unavailable(String),
}

pub fn acquire_launch_lock(identifier: &str) -> LaunchLockState {
    let Some(dir) = crate::db::desktop_db_dir(identifier) else {
        return LaunchLockState::Unavailable(
            "application data directory is unavailable".to_string(),
        );
    };
    if let Err(error) = std::fs::create_dir_all(&dir) {
        return LaunchLockState::Unavailable(format!(
            "failed to create application data directory: {error}"
        ));
    }
    lock_launch_file(&dir.join(LAUNCH_LOCK_FILENAME))
}

fn lock_launch_file(path: &Path) -> LaunchLockState {
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) => {
            return LaunchLockState::Unavailable(format!(
                "failed to open {}: {error}",
                path.display()
            ));
        }
    };

    match file.try_lock() {
        Ok(()) => LaunchLockState::Acquired(LaunchLock { _file: file }),
        Err(TryLockError::WouldBlock) => LaunchLockState::HeldByAnotherProcess,
        Err(TryLockError::Error(error)) => {
            LaunchLockState::Unavailable(format!("failed to lock {}: {error}", path.display()))
        }
    }
}

pub fn exit_for_already_running_instance() -> ! {
    eprintln!("another Anarlog process holds the launch lock; exiting");

    #[cfg(target_os = "macos")]
    {
        let alert = "display alert \"Anarlog is already starting\" message \"Another Anarlog process is preparing your data, possibly finishing an update. The app will open automatically when it is ready.\" buttons {\"OK\"} default button \"OK\"";
        let _ = std::process::Command::new("/usr/bin/osascript")
            .args(["-e", alert])
            .spawn();
    }

    std::process::exit(0);
}

// Database migrations run before the webview or dock icon exists, so a long
// migration makes the app look dead. After a short delay this shows a native
// alert (spawned osascript, matching the startup-failure alerts) that is
// killed as soon as the database is ready.
pub struct SlowStartupIndicator {
    state: Arc<Mutex<IndicatorState>>,
}

struct IndicatorState {
    dismissed: bool,
    child: Option<std::process::Child>,
}

impl SlowStartupIndicator {
    pub fn show_after_delay() -> Self {
        let state = Arc::new(Mutex::new(IndicatorState {
            dismissed: false,
            child: None,
        }));

        {
            let state = state.clone();
            std::thread::spawn(move || {
                std::thread::sleep(SLOW_STARTUP_INDICATOR_DELAY);
                let mut state = state.lock().unwrap();
                if state.dismissed {
                    return;
                }
                state.child = spawn_indicator_alert();
            });
        }

        Self { state }
    }

    pub fn dismiss(&self) {
        let mut state = self.state.lock().unwrap();
        state.dismissed = true;
        if let Some(mut child) = state.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(target_os = "macos")]
fn spawn_indicator_alert() -> Option<std::process::Child> {
    let alert = "display alert \"Updating your data\" message \"Anarlog is updating your data. This can take several minutes for a large library.\\n\\nPlease keep Anarlog running; it will open automatically when the update finishes.\" buttons {\"OK\"} default button \"OK\"";
    std::process::Command::new("/usr/bin/osascript")
        .args(["-e", alert])
        .spawn()
        .ok()
}

#[cfg(not(target_os = "macos"))]
fn spawn_indicator_alert() -> Option<std::process::Child> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_lock_excludes_a_second_holder_until_released() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LAUNCH_LOCK_FILENAME);

        let first = lock_launch_file(&path);
        assert!(matches!(first, LaunchLockState::Acquired(_)));

        assert!(matches!(
            lock_launch_file(&path),
            LaunchLockState::HeldByAnotherProcess
        ));

        drop(first);
        assert!(matches!(
            lock_launch_file(&path),
            LaunchLockState::Acquired(_)
        ));
    }

    #[test]
    fn dismissing_before_the_delay_prevents_the_indicator() {
        let indicator = SlowStartupIndicator::show_after_delay();
        indicator.dismiss();

        let state = indicator.state.lock().unwrap();
        assert!(state.dismissed);
        assert!(state.child.is_none());
    }
}
