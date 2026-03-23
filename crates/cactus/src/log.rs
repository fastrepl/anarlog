use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::Mutex;

static COLLECTED_ERRORS: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn init() {
    unsafe {
        cactus_sys::cactus_log_set_level(3); // ERROR only
        cactus_sys::cactus_log_set_callback(Some(error_collector), std::ptr::null_mut());
    }
}

pub fn drain_errors() -> Vec<String> {
    std::mem::take(&mut *COLLECTED_ERRORS.lock().unwrap())
}

unsafe extern "C" fn error_collector(
    _level: c_int,
    component: *const c_char,
    message: *const c_char,
    _user_data: *mut c_void,
) {
    let component = unsafe { CStr::from_ptr(component) }.to_str().unwrap_or("");
    let message = unsafe { CStr::from_ptr(message) }.to_str().unwrap_or("");

    let entry = if component.is_empty() {
        message.to_string()
    } else {
        format!("[{component}] {message}")
    };

    if let Ok(mut errors) = COLLECTED_ERRORS.lock() {
        errors.push(entry);
    }
}
