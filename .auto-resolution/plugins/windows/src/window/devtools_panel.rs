use crate::Error;

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use std::sync::OnceLock;

    use swift_rs::{Bool, swift};
    use tauri_specta::Event;

    use crate::Error;

    swift!(fn _devtools_panel_show() -> Bool);
    swift!(fn _devtools_panel_hide() -> Bool);

    static APP_HANDLE: OnceLock<tauri::AppHandle<tauri::Wry>> = OnceLock::new();

    pub fn set_app_handle(app: tauri::AppHandle<tauri::Wry>) {
        let _ = APP_HANDLE.set(app);
    }

    pub fn show() -> Result<(), Error> {
        unsafe {
            _devtools_panel_show();
        }
        Ok(())
    }

    pub fn hide() -> Result<(), Error> {
        unsafe {
            _devtools_panel_hide();
        }
        Ok(())
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn rust_on_devtools_panel_action(action_ptr: *const c_char) {
        if action_ptr.is_null() {
            return;
        }

        let Ok(action) = unsafe { CStr::from_ptr(action_ptr) }.to_str() else {
            return;
        };

        if let Some(app) = APP_HANDLE.get() {
            let _ = crate::events::DevtoolsPanelAction {
                action: action.to_string(),
            }
            .emit(app);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use crate::Error;

    pub fn show() -> Result<(), Error> {
        Ok(())
    }

    pub fn hide() -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use platform::set_app_handle;

pub fn show() -> Result<(), Error> {
    platform::show()
}

pub fn hide() -> Result<(), Error> {
    platform::hide()
}
