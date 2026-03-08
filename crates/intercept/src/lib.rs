#[cfg(target_os = "macos")]
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

#[cfg(target_os = "macos")]
use swift_rs::swift;

#[cfg(target_os = "macos")]
swift!(fn _setup_force_quit_handler());

#[cfg(target_os = "macos")]
swift!(fn _show_quit_overlay());

#[cfg(target_os = "macos")]
swift!(fn _demo_quit_progress());

#[cfg(target_os = "macos")]
static HANDLER_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
static FORCE_QUIT: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
static CLOSE_HANDLER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

#[cfg(target_os = "macos")]
pub fn setup_force_quit_handler() {
    if !HANDLER_INITIALIZED.swap(true, Ordering::SeqCst) {
        unsafe {
            _setup_force_quit_handler();
        }
    }
}

#[cfg(target_os = "macos")]
pub fn set_close_handler(f: impl Fn() + Send + Sync + 'static) {
    let _ = CLOSE_HANDLER.set(Box::new(f));
}

#[cfg(target_os = "macos")]
pub fn should_force_quit() -> bool {
    FORCE_QUIT.load(Ordering::SeqCst)
}

#[cfg(target_os = "macos")]
pub fn set_force_quit() {
    FORCE_QUIT.store(true, Ordering::SeqCst);
}

#[cfg(target_os = "macos")]
pub fn show_quit_overlay() {
    unsafe {
        _show_quit_overlay();
    }
}

#[cfg(target_os = "macos")]
pub fn demo_quit_progress() {
    unsafe {
        _demo_quit_progress();
    }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "macos")]
pub extern "C" fn rust_set_force_quit() {
    FORCE_QUIT.store(true, Ordering::SeqCst);
}

#[unsafe(no_mangle)]
#[cfg(target_os = "macos")]
pub extern "C" fn rust_perform_close() {
    if let Some(handler) = CLOSE_HANDLER.get() {
        handler();
    }
}
