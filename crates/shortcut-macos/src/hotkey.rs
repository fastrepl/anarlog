use std::time::Instant;

use tokio::sync::mpsc;

#[derive(Debug)]
pub enum ShortcutEvent {
    RecordStart,
    RecordStop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutErrorKind {
    Unsupported,
    AccessibilityDenied,
    InputMonitoringDenied,
    SecureKeyboardEntry,
    TapDisabled,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutError {
    kind: ShortcutErrorKind,
    message: &'static str,
}

impl ShortcutError {
    pub fn internal(message: &'static str) -> Self {
        internal_error(message)
    }

    pub fn kind(self) -> ShortcutErrorKind {
        self.kind
    }

    pub fn message(self) -> &'static str {
        self.message
    }

    pub fn recovery(self) -> &'static str {
        match self.kind {
            ShortcutErrorKind::Unsupported => "This feature is only available on macOS.",
            ShortcutErrorKind::AccessibilityDenied => {
                "Open System Settings → Privacy & Security → Accessibility, enable char, and retry."
            }
            ShortcutErrorKind::InputMonitoringDenied => {
                "Open System Settings → Privacy & Security → Input Monitoring, enable char, and retry."
            }
            ShortcutErrorKind::SecureKeyboardEntry => {
                "Disable Secure Keyboard Entry in the app holding it, then retry."
            }
            ShortcutErrorKind::TapDisabled => {
                "Inspect the shortcut stderr log, then retry `char shortcut install`."
            }
            ShortcutErrorKind::Internal => "Inspect the shortcut stderr log and retry.",
        }
    }
}

pub fn current_blocker() -> Option<ShortcutError> {
    #[cfg(target_os = "macos")]
    {
        if !accessibility_trusted() {
            return Some(accessibility_error());
        }
        if secure_keyboard_entry_enabled() {
            return Some(secure_keyboard_entry_error());
        }
        probe_event_tap().err()
    }

    #[cfg(not(target_os = "macos"))]
    {
        Some(unsupported_error())
    }
}

pub fn input_monitoring_granted() -> bool {
    #[cfg(target_os = "macos")]
    {
        probe_event_tap().is_ok()
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn run_listener_on_main_thread(
    event_tx: mpsc::UnboundedSender<ShortcutEvent>,
) -> Result<(), ShortcutError> {
    #[cfg(target_os = "macos")]
    // The daemon path must enter the real main-thread/AppKit listener directly.
    // A lighter preflight probe before this point produced false negatives under
    // launchd, because macOS would reject the tap before AppKit/main-run-loop bootstrap.
    unsafe {
        run_event_tap_on_main_thread(event_tx)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = event_tx;
        Err(unsupported_error())
    }
}

fn secure_keyboard_entry_error() -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::SecureKeyboardEntry,
        message: "Secure Keyboard Entry is enabled and blocks the global hotkey.",
    }
}

fn accessibility_error() -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::AccessibilityDenied,
        message: "Accessibility permission is required for the global hotkey daemon.",
    }
}

fn input_monitoring_error() -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::InputMonitoringDenied,
        message: "Input Monitoring permission is required for the global hotkey.",
    }
}

fn tap_disabled_error(message: &'static str) -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::TapDisabled,
        message,
    }
}

fn internal_error(message: &'static str) -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::Internal,
        message,
    }
}

#[cfg(not(target_os = "macos"))]
fn unsupported_error() -> ShortcutError {
    ShortcutError {
        kind: ShortcutErrorKind::Unsupported,
        message: "Global shortcuts are only supported on macOS.",
    }
}

const DOUBLE_TAP_WINDOW_MS: u128 = 400;
const RIGHT_ALT_MASK: u64 = 0x00000040;

#[cfg(target_os = "macos")]
const CGEVENT_TAP_DISABLED_BY_TIMEOUT: CGEventType = 0xFFFFFFFE;
#[cfg(target_os = "macos")]
const CGEVENT_TAP_DISABLED_BY_USER: CGEventType = 0xFFFFFFFF;

#[cfg(target_os = "macos")]
enum TapState {
    Idle,
    FirstTap(Instant),
    Recording,
}

#[cfg(target_os = "macos")]
fn probe_event_tap() -> Result<(), ShortcutError> {
    unsafe extern "C" fn noop_callback(
        _proxy: CGEventTapProxy,
        _event_type: CGEventType,
        event: CGEventRef,
        _user_info: *mut std::ffi::c_void,
    ) -> CGEventRef {
        event
    }

    let event_mask = (1 << CGEVENT_FLAGS_CHANGED) as CGEventMask;
    let tap = unsafe {
        CGEventTapCreate(
            KCGSESSION_EVENT_TAP,
            KCGHEAD_INSERT_EVENT_TAP,
            KCGEVENT_TAP_OPTION_LISTEN_ONLY,
            event_mask,
            noop_callback,
            std::ptr::null_mut(),
        )
    };

    if tap.is_null() {
        return Err(input_monitoring_error());
    }
    let run_loop_source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0) };
    if run_loop_source.is_null() {
        unsafe { CFRelease(tap) };
        return Err(tap_disabled_error(
            "Failed to create a run loop source for the event tap.",
        ));
    }

    unsafe {
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        let result = if CGEventTapIsEnabled(tap) {
            Ok(())
        } else {
            Err(tap_disabled_error(
                "macOS created the event tap but left it disabled.",
            ))
        };
        CFRunLoopRemoveSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
        CFRelease(run_loop_source);
        CFRelease(tap);
        result
    }
}

#[cfg(target_os = "macos")]
unsafe fn run_event_tap_on_main_thread(
    event_tx: mpsc::UnboundedSender<ShortcutEvent>,
) -> Result<(), ShortcutError> {
    if unsafe { pthread_main_np() } != 1 {
        return Err(internal_error(
            "Shortcut listener must run on the process main thread.",
        ));
    }

    bootstrap_appkit();

    if !accessibility_trusted() {
        return Err(accessibility_error());
    }

    if secure_keyboard_entry_enabled() {
        return Err(secure_keyboard_entry_error());
    }

    let mut state = TapState::Idle;
    let mut was_pressed = false;
    let mut runtime_error: Option<ShortcutError> = None;

    let state_ptr = &mut state as *mut TapState;
    let was_pressed_ptr = &mut was_pressed as *mut bool;
    let event_tx_ptr = &event_tx as *const mpsc::UnboundedSender<ShortcutEvent>;
    let runtime_error_ptr = &mut runtime_error as *mut Option<ShortcutError>;

    struct CallbackData {
        state: *mut TapState,
        was_pressed: *mut bool,
        event_tx: *const mpsc::UnboundedSender<ShortcutEvent>,
        runtime_error: *mut Option<ShortcutError>,
    }

    let mut data = CallbackData {
        state: state_ptr,
        was_pressed: was_pressed_ptr,
        event_tx: event_tx_ptr,
        runtime_error: runtime_error_ptr,
    };

    unsafe extern "C" fn callback(
        _proxy: CGEventTapProxy,
        event_type: CGEventType,
        event: CGEventRef,
        user_info: *mut std::ffi::c_void,
    ) -> CGEventRef {
        let data = unsafe { &mut *(user_info as *mut CallbackData) };

        if event_type == CGEVENT_TAP_DISABLED_BY_TIMEOUT
            || event_type == CGEVENT_TAP_DISABLED_BY_USER
        {
            let runtime_error = unsafe { &mut *data.runtime_error };
            if runtime_error.is_none() {
                *runtime_error = Some(tap_disabled_error(
                    "macOS disabled the event tap while the daemon was running.",
                ));
            }
            unsafe { CFRunLoopStop(CFRunLoopGetMain()) };
            return event;
        }

        let flags = unsafe { CGEventGetFlags(event) };
        let is_pressed = (flags & RIGHT_ALT_MASK) != 0;
        let was = unsafe { *data.was_pressed };

        if is_pressed && !was {
            let state = unsafe { &mut *data.state };
            let event_tx = unsafe { &*data.event_tx };

            match state {
                TapState::Idle => *state = TapState::FirstTap(Instant::now()),
                TapState::FirstTap(first) => {
                    if first.elapsed().as_millis() <= DOUBLE_TAP_WINDOW_MS {
                        let _ = event_tx.send(ShortcutEvent::RecordStart);
                        *state = TapState::Recording;
                    } else {
                        *state = TapState::FirstTap(Instant::now());
                    }
                }
                TapState::Recording => {
                    let _ = event_tx.send(ShortcutEvent::RecordStop);
                    *state = TapState::Idle;
                }
            }
        }

        unsafe { *data.was_pressed = is_pressed };
        event
    }

    let event_mask = (1 << CGEVENT_FLAGS_CHANGED) as CGEventMask;
    let tap = unsafe {
        CGEventTapCreate(
            KCGSESSION_EVENT_TAP,
            KCGHEAD_INSERT_EVENT_TAP,
            KCGEVENT_TAP_OPTION_LISTEN_ONLY,
            event_mask,
            callback,
            &mut data as *mut CallbackData as *mut std::ffi::c_void,
        )
    };

    if tap.is_null() {
        return Err(input_monitoring_error());
    }

    let run_loop_source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0) };
    if run_loop_source.is_null() {
        unsafe { CFRelease(tap) };
        return Err(tap_disabled_error(
            "Failed to create a run loop source for the event tap.",
        ));
    }

    unsafe {
        let run_loop = CFRunLoopGetMain();
        CFRunLoopAddSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);

        if !CGEventTapIsEnabled(tap) {
            CFRunLoopRemoveSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
            CFMachPortInvalidate(tap);
            CFRelease(run_loop_source);
            CFRelease(tap);
            return Err(tap_disabled_error(
                "macOS created the event tap but left it disabled.",
            ));
        }

        CFRunLoopRun();
        CFRunLoopRemoveSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
        CFMachPortInvalidate(tap);
        CFRelease(run_loop_source);
        CFRelease(tap);
    }

    if let Some(error) = runtime_error {
        return Err(error);
    }

    Err(internal_error("Hotkey listener exited unexpectedly."))
}

#[cfg(target_os = "macos")]
fn secure_keyboard_entry_enabled() -> bool {
    unsafe { CGSIsSecureEventInputSet() }
}

#[cfg(target_os = "macos")]
fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
fn bootstrap_appkit() {
    unsafe {
        let _ = NSApplicationLoad();
    }
}

#[cfg(target_os = "macos")]
type CGEventTapProxy = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CGEventRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CGEventMask = u64;
#[cfg(target_os = "macos")]
type CGEventType = u32;
#[cfg(target_os = "macos")]
type CFMachPortRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFRunLoopSourceRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFRunLoopRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFAllocatorRef = *const std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFStringRef = *const std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFIndex = isize;

#[cfg(target_os = "macos")]
const KCGSESSION_EVENT_TAP: u32 = 1;
#[cfg(target_os = "macos")]
const KCGHEAD_INSERT_EVENT_TAP: u32 = 0;
#[cfg(target_os = "macos")]
const KCGEVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
#[cfg(target_os = "macos")]
const CGEVENT_FLAGS_CHANGED: u32 = 12;

#[cfg(target_os = "macos")]
type CGEventTapCallBack = unsafe extern "C" fn(
    CGEventTapProxy,
    CGEventType,
    CGEventRef,
    *mut std::ffi::c_void,
) -> CGEventRef;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: CGEventMask,
        callback: CGEventTapCallBack,
        user_info: *mut std::ffi::c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventTapIsEnabled(tap: CFMachPortRef) -> bool;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    fn CFMachPortInvalidate(port: CFMachPortRef);
    fn CFRunLoopGetMain() -> CFRunLoopRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRemoveSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn pthread_main_np() -> i32;

    static kCFRunLoopCommonModes: CFStringRef;

    fn CFRelease(cf: *mut std::ffi::c_void);
    fn CGSIsSecureEventInputSet() -> bool;
    fn AXIsProcessTrusted() -> bool;
}

#[cfg(target_os = "macos")]
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    fn NSApplicationLoad() -> libc::c_char;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessibility_recovery_points_to_accessibility_settings() {
        let error = accessibility_error();
        assert_eq!(error.kind(), ShortcutErrorKind::AccessibilityDenied);
        assert!(error.recovery().contains("Accessibility"));
    }

    #[test]
    fn input_monitoring_recovery_points_to_input_monitoring() {
        let error = input_monitoring_error();
        assert_eq!(error.kind(), ShortcutErrorKind::InputMonitoringDenied);
        assert!(error.recovery().contains("Input Monitoring"));
    }

    #[test]
    fn secure_keyboard_entry_recovery_mentions_secure_keyboard_entry() {
        let error = secure_keyboard_entry_error();
        assert_eq!(error.kind(), ShortcutErrorKind::SecureKeyboardEntry);
        assert!(error.recovery().contains("Secure Keyboard Entry"));
    }

    #[test]
    fn internal_recovery_stays_generic() {
        let error = ShortcutError::internal("boom");
        assert_eq!(error.kind(), ShortcutErrorKind::Internal);
        assert!(error.recovery().contains("Inspect the shortcut stderr log"));
    }
}
