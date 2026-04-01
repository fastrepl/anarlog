use std::sync::Mutex;

type Callback = Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>;
type OptionCallback = Mutex<Option<Box<dyn Fn(String, i32) + Send + Sync>>>;

static CONFIRM_CB: Callback = Mutex::new(None);
static ACCEPT_CB: Callback = Mutex::new(None);
static DISMISS_CB: Callback = Mutex::new(None);
static TIMEOUT_CB: Callback = Mutex::new(None);
static OPTION_SELECTED_CB: OptionCallback = Mutex::new(None);

pub fn setup_confirm_handler<F: Fn(String) + Send + Sync + 'static>(f: F) {
    *CONFIRM_CB.lock().unwrap() = Some(Box::new(f));
}

pub fn setup_accept_handler<F: Fn(String) + Send + Sync + 'static>(f: F) {
    *ACCEPT_CB.lock().unwrap() = Some(Box::new(f));
}

pub fn setup_dismiss_handler<F: Fn(String) + Send + Sync + 'static>(f: F) {
    *DISMISS_CB.lock().unwrap() = Some(Box::new(f));
}

pub fn setup_timeout_handler<F: Fn(String) + Send + Sync + 'static>(f: F) {
    *TIMEOUT_CB.lock().unwrap() = Some(Box::new(f));
}

pub fn setup_option_selected_handler<F: Fn(String, i32) + Send + Sync + 'static>(f: F) {
    *OPTION_SELECTED_CB.lock().unwrap() = Some(Box::new(f));
}

pub(crate) fn fire_confirm(key: String) {
    if let Some(cb) = CONFIRM_CB.lock().unwrap().as_ref() {
        cb(key);
    }
}

pub(crate) fn fire_accept(key: String) {
    if let Some(cb) = ACCEPT_CB.lock().unwrap().as_ref() {
        cb(key);
    }
}

pub(crate) fn fire_dismiss(key: String) {
    if let Some(cb) = DISMISS_CB.lock().unwrap().as_ref() {
        cb(key);
    }
}

pub(crate) fn fire_timeout(key: String) {
    if let Some(cb) = TIMEOUT_CB.lock().unwrap().as_ref() {
        cb(key);
    }
}

pub(crate) fn fire_option_selected(key: String, index: i32) {
    if let Some(cb) = OPTION_SELECTED_CB.lock().unwrap().as_ref() {
        cb(key, index);
    }
}
