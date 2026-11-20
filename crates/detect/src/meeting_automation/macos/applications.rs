use std::{
    sync::{Mutex, MutexGuard, PoisonError},
    thread,
    time::{Duration, Instant},
};

use anlg_overlay_kit::macos::main_thread::run_on_main_thread;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
use objc2_application_services::{AXIsProcessTrusted, AXUIElement};
use objc2_core_foundation::CFBoolean;

use super::ax::{perform, set_attribute};

/// Held while Anarlog drives a meeting app (posting the chat notice, switching the camera), so two
/// such actions never take the front or press keys at the same time.
pub(crate) static MEETING_APP_LOCK: Mutex<()> = Mutex::new(());

const FRONT_POLL: Duration = Duration::from_millis(100);
const FRONT_TIMEOUT: Duration = Duration::from_millis(2_000);

/// Takes the meeting-app lock. A panic in another holder leaves no state behind, so a poisoned
/// lock is still usable.
pub(crate) fn lock_meeting_app() -> MutexGuard<'static, ()> {
    MEETING_APP_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn is_accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// The bundle identifier of a running process, for telling which app a call is in.
pub(crate) fn bundle_id_of(pid: i32) -> Option<String> {
    if pid <= 0 {
        return None;
    }
    run_on_main_thread(move || {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .filter(|app| !app.isTerminated())
            .and_then(|app| app.bundleIdentifier())
            .map(|id| id.to_string())
    })
}

pub(crate) fn frontmost_pid() -> Option<i32> {
    run_on_main_thread(|| {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| app.processIdentifier())
    })
}

/// Brings an app and its window to the front, for the keys that follow; false if it didn't come.
pub(crate) fn bring_to_front(
    pid: i32,
    application: &AXUIElement,
    window: Option<&AXUIElement>,
) -> bool {
    if frontmost_pid() != Some(pid) {
        set_attribute(application, "AXFrontmost", CFBoolean::new(true).as_ref());
    }
    if let Some(window) = window {
        perform(window, "AXRaise");
        set_attribute(window, "AXMain", CFBoolean::new(true).as_ref());
    }
    let deadline = Instant::now() + FRONT_TIMEOUT;
    while frontmost_pid() != Some(pid) {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(FRONT_POLL);
    }
    true
}

/// Gives the front back to the app that had it before Anarlog brought `pid` forward, unless the
/// person moved on meanwhile.
pub(crate) fn give_front_back(previous: Option<i32>, pid: i32) {
    if let Some(previous) = previous
        && previous != pid
        && frontmost_pid() == Some(pid)
    {
        activate(previous);
    }
}

/// Asks AppKit to activate `pid`. Setting `AXFrontmost` asks the app to activate itself, which
/// apps linked against macOS 14 or later refuse while the meeting app holds the front (cooperative
/// activation), so the front is handed back this way.
fn activate(pid: i32) {
    run_on_main_thread(move || {
        if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
            app.activateWithOptions(NSApplicationActivationOptions::empty());
        }
    });
}
