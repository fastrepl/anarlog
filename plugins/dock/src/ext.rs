#[cfg(target_os = "macos")]
pub fn setup_dock_menu(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::{msg_send, sel};
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    crate::APP_HANDLE.get_or_init(|| app.clone());

    app.run_on_main_thread(move || {
        let mtm = MainThreadMarker::new().expect("run_on_main_thread guarantees main thread");
        let ns_app = NSApplication::sharedApplication(mtm);

        unsafe {
            let delegate: *mut AnyObject = msg_send![&*ns_app, delegate];
            if delegate.is_null() {
                return;
            }

            let delegate_class: *mut AnyClass = msg_send![delegate, class];
            if delegate_class.is_null() {
                return;
            }

            let sel = sel!(applicationDockMenu:);

            extern "C" fn dock_menu_handler(
                _this: *mut objc2::runtime::AnyObject,
                _sel: objc2::runtime::Sel,
                _sender: *mut objc2::runtime::AnyObject,
            ) -> *mut objc2::runtime::AnyObject {
                let mtm = unsafe { objc2_foundation::MainThreadMarker::new_unchecked() };

                let ns_app = objc2_app_kit::NSApplication::sharedApplication(mtm);
                let windows = ns_app.windows();
                for i in 0..windows.len() {
                    let window: *mut objc2::runtime::AnyObject =
                        unsafe { objc2::msg_send![&*windows, objectAtIndex: i] };
                    if !window.is_null() {
                        let _: () =
                            unsafe { objc2::msg_send![window, setExcludedFromWindowsMenu: true] };
                    }
                }

                let menu = crate::menu_items::build_dock_menu(mtm);
                objc2::rc::Retained::autorelease_return(menu) as *mut objc2::runtime::AnyObject
            }

            let dock_imp: objc2::runtime::Imp = std::mem::transmute(dock_menu_handler as *const ());
            let dock_types = c"@@:@";

            let added =
                objc2::ffi::class_addMethod(delegate_class, sel, dock_imp, dock_types.as_ptr());
            if !added.as_bool() {
                objc2::ffi::class_replaceMethod(delegate_class, sel, dock_imp, dock_types.as_ptr());
            }

            crate::menu_items::register_action_handlers(delegate_class);
        }
    })?;

    Ok(())
}
