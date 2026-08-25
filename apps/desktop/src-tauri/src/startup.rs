use std::fs::TryLockError;
use std::path::{Path, PathBuf};
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

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const LAUNCHD_PID: u32 = 1;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const LAUNCH_SERVICES_MACH_SERVICE: &str = "com.apple.coreservices.launchservicesd";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppkitLaunchPlan {
    Continue,
    Relaunch,
    Exit,
}

fn appkit_launch_plan(
    parent_is_launchd: bool,
    launch_services_reachable: bool,
    has_app_bundle: bool,
) -> AppkitLaunchPlan {
    if parent_is_launchd || launch_services_reachable {
        return AppkitLaunchPlan::Continue;
    }
    if has_app_bundle {
        AppkitLaunchPlan::Relaunch
    } else {
        AppkitLaunchPlan::Exit
    }
}

fn app_bundle_path_from_executable(executable: &Path) -> Option<PathBuf> {
    let bundle = executable.parent()?.parent()?.parent()?;
    (bundle.extension().and_then(|ext| ext.to_str()) == Some("app")).then(|| bundle.to_path_buf())
}

// macOS AppKit abort()s in _RegisterApplication when this process cannot obtain
// an ASN from launchservicesd (typically an inherited sandbox from a parent
// such as a browser agent). Probe before EventLoop/NSApplication is created.
pub(crate) fn ensure_macos_appkit_launch_context() {
    #[cfg(target_os = "macos")]
    ensure_macos_appkit_launch_context_macos();
}

#[cfg(target_os = "macos")]
fn ensure_macos_appkit_launch_context_macos() {
    let bundle = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(app_bundle_path_from_executable);
    match appkit_launch_plan(
        process_parent_id() == LAUNCHD_PID,
        launch_services_reachable(),
        bundle.is_some(),
    ) {
        AppkitLaunchPlan::Continue => {}
        AppkitLaunchPlan::Relaunch => {
            if let Some(bundle) = bundle.as_deref()
                && relaunch_via_launch_services(bundle)
            {
                std::process::exit(0);
            }
            exit_for_unreachable_launch_services();
        }
        AppkitLaunchPlan::Exit => exit_for_unreachable_launch_services(),
    }
}

#[cfg(target_os = "macos")]
fn process_parent_id() -> u32 {
    // SAFETY: getppid is a POSIX query of this process's parent.
    (unsafe { libc_getppid() }) as u32
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    #[link_name = "getppid"]
    fn libc_getppid() -> i32;
}

#[cfg(target_os = "macos")]
fn launch_services_reachable() -> bool {
    !sandbox_denies_launch_services_lookup() && launch_services_mach_service_reachable()
}

#[cfg(target_os = "macos")]
fn sandbox_denies_launch_services_lookup() -> bool {
    let Ok(operation) = std::ffi::CString::new("mach-lookup") else {
        return false;
    };
    let Ok(name) = std::ffi::CString::new(LAUNCH_SERVICES_MACH_SERVICE) else {
        return false;
    };
    const SANDBOX_FILTER_GLOBAL_NAME: i32 = 2;
    const SANDBOX_CHECK_NO_REPORT: i32 = 1 << 16;

    // SAFETY: CStrings outlive the call; sandbox_check only reads the pointers.
    let result = unsafe {
        sandbox_check(
            std::process::id() as i32,
            operation.as_ptr(),
            SANDBOX_FILTER_GLOBAL_NAME | SANDBOX_CHECK_NO_REPORT,
            name.as_ptr(),
        )
    };
    result != 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::PermissionDenied
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn sandbox_check(
        pid: i32,
        operation: *const std::ffi::c_char,
        filter_type: i32,
        name: *const std::ffi::c_char,
    ) -> i32;
}

#[cfg(target_os = "macos")]
fn launch_services_mach_service_reachable() -> bool {
    let Ok(name) = std::ffi::CString::new(LAUNCH_SERVICES_MACH_SERVICE) else {
        return false;
    };
    let mut port = 0u32;
    // SAFETY: bootstrap_port is the process bootstrap port; name is a valid
    // C string; we deallocate any returned send right.
    let kr = unsafe { bootstrap_look_up(bootstrap_port, name.as_ptr(), &mut port) };
    if kr == 0 && port != 0 {
        unsafe {
            let _ = mach_port_deallocate(mach_task_self_, port);
        }
        true
    } else {
        false
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    static bootstrap_port: u32;
    static mach_task_self_: u32;
    fn bootstrap_look_up(bp: u32, service_name: *const std::ffi::c_char, sp: *mut u32) -> i32;
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
}

#[cfg(target_os = "macos")]
fn relaunch_via_launch_services(bundle: &Path) -> bool {
    let mut command = std::process::Command::new("/usr/bin/open");
    command.arg(bundle);
    let extra_args: Vec<_> = std::env::args_os().skip(1).collect();
    if !extra_args.is_empty() {
        command.arg("--args").args(extra_args);
    }
    command
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn exit_for_unreachable_launch_services() -> ! {
    eprintln!(
        "Anarlog could not start because this environment cannot register a Mac app with Launch Services. Open Anarlog from the Dock, Spotlight, or Finder."
    );

    let alert = "display alert \"Anarlog could not start\" message \"This environment cannot open Anarlog as a graphical app. Please open Anarlog from the Dock, Spotlight, or Finder.\" as critical buttons {\"OK\"} default button \"OK\"";
    let _ = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", alert])
        .spawn();

    std::process::exit(1);
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
    fn dismissing_before_the_delay_prevents_the_indicator() {
        let indicator = SlowStartupIndicator::show_after_delay();
        indicator.dismiss();

        let state = indicator.state.lock().unwrap();
        assert!(state.dismissed);
        assert!(state.child.is_none());
    }

    #[test]
    fn appkit_launch_plan_continues_when_parent_is_launchd() {
        assert_eq!(
            appkit_launch_plan(true, false, true),
            AppkitLaunchPlan::Continue
        );
    }

    #[test]
    fn appkit_launch_plan_continues_when_launch_services_is_reachable() {
        assert_eq!(
            appkit_launch_plan(false, true, false),
            AppkitLaunchPlan::Continue
        );
    }

    #[test]
    fn appkit_launch_plan_relaunches_a_bundle_when_launch_services_is_blocked() {
        assert_eq!(
            appkit_launch_plan(false, false, true),
            AppkitLaunchPlan::Relaunch
        );
    }

    #[test]
    fn appkit_launch_plan_exits_unpackaged_binaries_when_launch_services_is_blocked() {
        assert_eq!(
            appkit_launch_plan(false, false, false),
            AppkitLaunchPlan::Exit
        );
    }

    #[test]
    fn app_bundle_path_from_macos_executable() {
        assert_eq!(
            app_bundle_path_from_executable(Path::new(
                "/Applications/Anarlog.app/Contents/MacOS/anarlog"
            )),
            Some(PathBuf::from("/Applications/Anarlog.app"))
        );
    }

    #[test]
    fn app_bundle_path_ignores_unpackaged_executables() {
        assert_eq!(
            app_bundle_path_from_executable(Path::new("/tmp/target/debug/desktop")),
            None
        );
        assert_eq!(
            app_bundle_path_from_executable(Path::new("/Applications/Anarlog.app/Contents/MacOS")),
            None
        );
    }
}
