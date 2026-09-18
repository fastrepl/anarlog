use std::fs::TryLockError;
use std::path::Path;
use std::sync::{Arc, Mutex};

const LAUNCH_LOCK_FILENAME: &str = "launch.lock";
const SLOW_STARTUP_INDICATOR_DELAY: std::time::Duration = std::time::Duration::from_secs(3);
const CRASH_REPORTER_SERVER_ARG: &str = "--crash-reporter-server";
const WEBKIT_DISABLE_DMABUF_RENDERER: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

// WebKitGTK's DMA-BUF renderer leaves a blank window on NVIDIA/Wayland and
// some other GPU/compositor combinations. Set the fallback before Tokio or
// the webview start, and leave an explicit user override alone.
pub(crate) fn apply_linux_webkit_workarounds() {
    if !cfg!(target_os = "linux") {
        return;
    }

    if let Some(value) =
        linux_webkit_dmabuf_override(std::env::var_os(WEBKIT_DISABLE_DMABUF_RENDERER).as_deref())
    {
        // SAFETY: called from the process entrypoint before other threads start.
        unsafe {
            std::env::set_var(WEBKIT_DISABLE_DMABUF_RENDERER, value);
        }
    }
}

fn linux_webkit_dmabuf_override(existing: Option<&std::ffi::OsStr>) -> Option<&'static str> {
    existing.is_none().then_some("1")
}

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

pub fn is_crash_reporter_process() -> bool {
    std::env::args().any(|arg| is_crash_reporter_arg(&arg))
}

fn is_crash_reporter_arg(arg: &str) -> bool {
    arg.starts_with(CRASH_REPORTER_SERVER_ARG)
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

// Nightly and stable open the same database. SQLite serializes their writes,
// but live queries and CloudSync only see their own process, so the two
// channels exclude each other for the whole app lifetime. Each channel holds
// its own lock file; the peer's lock being held means the peer is running.
// Same-channel relaunches keep working: an own lock that is already held is
// left to the single-instance plugin, as before.
pub struct ChannelLock {
    _file: std::fs::File,
}

pub enum ChannelLockState {
    Acquired(Option<ChannelLock>),
    PeerRunning { peer: &'static str },
    Unavailable(String),
}

pub fn acquire_channel_lock(identifier: &str) -> ChannelLockState {
    let Some(peer) = crate::db::shared_database_peer(identifier) else {
        return ChannelLockState::Acquired(None);
    };
    let Some(dir) = crate::db::desktop_db_dir(identifier) else {
        return ChannelLockState::Unavailable(
            "application data directory is unavailable".to_string(),
        );
    };
    lock_channel_files(
        &dir.join(channel_lock_filename(identifier)),
        &dir.join(channel_lock_filename(peer)),
        peer,
    )
}

fn channel_lock_filename(identifier: &str) -> String {
    format!("{identifier}.running.lock")
}

// The peer is probed before our own lock is taken. Two channels starting at
// the same instant can both slip through, which only preserves today's
// behavior; the reverse order could make both of them exit.
fn lock_channel_files(own_path: &Path, peer_path: &Path, peer: &'static str) -> ChannelLockState {
    match lock_launch_file(peer_path) {
        LaunchLockState::Acquired(probe) => drop(probe),
        LaunchLockState::HeldByAnotherProcess => return ChannelLockState::PeerRunning { peer },
        LaunchLockState::Unavailable(reason) => return ChannelLockState::Unavailable(reason),
    }

    match lock_launch_file(own_path) {
        LaunchLockState::Acquired(LaunchLock { _file }) => {
            ChannelLockState::Acquired(Some(ChannelLock { _file }))
        }
        LaunchLockState::HeldByAnotherProcess => ChannelLockState::Acquired(None),
        LaunchLockState::Unavailable(reason) => ChannelLockState::Unavailable(reason),
    }
}

pub fn exit_for_running_peer_channel(identifier: &str, peer: &str) -> ! {
    let own_name = channel_product_name(identifier);
    let peer_name = channel_product_name(peer);
    eprintln!("{peer_name} is running on the shared database; exiting {own_name}");

    #[cfg(target_os = "macos")]
    {
        let alert = format!(
            "display alert \"{own_name} cannot open yet\" message \"{peer_name} is running, and both apps use the same notes. Quit {peer_name}, then open {own_name} again.\" as critical buttons {{\"OK\"}} default button \"OK\""
        );
        let _ = std::process::Command::new("/usr/bin/osascript")
            .args(["-e", &alert])
            .spawn();
    }

    std::process::exit(0);
}

fn channel_product_name(identifier: &str) -> &'static str {
    if identifier == crate::db::NIGHTLY_BUNDLE_ID {
        "BlackMushi Nightly"
    } else {
        "BlackMushi"
    }
}

pub fn exit_for_already_running_instance() -> ! {
    eprintln!("another BlackMushi process holds the launch lock; exiting");

    #[cfg(target_os = "macos")]
    {
        let alert = "display alert \"BlackMushi is already starting\" message \"Another BlackMushi process is preparing your data, possibly finishing an update. The app will open automatically when it is ready.\" buttons {\"OK\"} default button \"OK\"";
        let _ = std::process::Command::new("/usr/bin/osascript")
            .args(["-e", alert])
            .spawn();
    }

    std::process::exit(0);
}

// Database open and plugin setup can still take a few seconds before the
// webview exists. After a short delay this shows a native alert (spawned
// osascript, matching the startup-failure alerts) that is killed as soon as
// the database file is open. Longer schema and legacy-import work then runs
// with the main window visible.
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
    let alert = "display alert \"Updating your data\" message \"BlackMushi is updating your data. This can take several minutes for a large library.\\n\\nPlease keep BlackMushi running; it will open automatically when the update finishes.\" buttons {\"OK\"} default button \"OK\"";
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
    fn linux_webkit_workaround_defaults_dmabuf_off_when_unset() {
        assert_eq!(linux_webkit_dmabuf_override(None), Some("1"));
    }

    #[test]
    fn linux_webkit_workaround_preserves_an_explicit_override() {
        assert_eq!(
            linux_webkit_dmabuf_override(Some(std::ffi::OsStr::new("0"))),
            None
        );
        assert_eq!(
            linux_webkit_dmabuf_override(Some(std::ffi::OsStr::new("1"))),
            None
        );
    }

    #[test]
    fn crash_reporter_args_are_detected() {
        assert!(is_crash_reporter_arg(
            "--crash-reporter-server=/tmp/temp-socket-abc"
        ));
        assert!(is_crash_reporter_arg("--crash-reporter-server"));
        assert!(!is_crash_reporter_arg("--background"));
        assert!(!is_crash_reporter_arg("--crash-reporter"));
    }

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
    fn channel_lock_excludes_the_peer_channel_while_held() {
        let dir = tempfile::tempdir().unwrap();
        let stable = dir
            .path()
            .join(channel_lock_filename("com.hyprnote.stable"));
        let nightly = dir
            .path()
            .join(channel_lock_filename("com.hyprnote.nightly"));

        let stable_lock = lock_channel_files(&stable, &nightly, "com.hyprnote.nightly");
        assert!(matches!(stable_lock, ChannelLockState::Acquired(Some(_))));

        assert!(matches!(
            lock_channel_files(&nightly, &stable, "com.hyprnote.stable"),
            ChannelLockState::PeerRunning {
                peer: "com.hyprnote.stable"
            }
        ));

        drop(stable_lock);
        assert!(matches!(
            lock_channel_files(&nightly, &stable, "com.hyprnote.stable"),
            ChannelLockState::Acquired(Some(_))
        ));
    }

    #[test]
    fn channel_lock_defers_same_channel_relaunches_to_single_instance() {
        let dir = tempfile::tempdir().unwrap();
        let stable = dir
            .path()
            .join(channel_lock_filename("com.hyprnote.stable"));
        let nightly = dir
            .path()
            .join(channel_lock_filename("com.hyprnote.nightly"));

        let _running = lock_channel_files(&stable, &nightly, "com.hyprnote.nightly");

        assert!(matches!(
            lock_channel_files(&stable, &nightly, "com.hyprnote.nightly"),
            ChannelLockState::Acquired(None)
        ));
    }

    #[test]
    fn channels_without_a_shared_database_skip_the_channel_lock() {
        assert!(matches!(
            acquire_channel_lock("com.hyprnote.staging"),
            ChannelLockState::Acquired(None)
        ));
    }

    #[test]
    fn channel_product_names_follow_the_bundle_identifier() {
        assert_eq!(
            channel_product_name("com.hyprnote.nightly"),
            "BlackMushi Nightly"
        );
        assert_eq!(channel_product_name("com.hyprnote.stable"), "BlackMushi");
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
