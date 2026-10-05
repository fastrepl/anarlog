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
            register_termination_handler(delegate_class);
            register_system_termination_observer(&*delegate);
        }
    })?;

    Ok(())
}

unsafe fn register_termination_handler(delegate_class: *mut objc2::runtime::AnyClass) {
    use objc2::{runtime::Imp, sel};

    let selector = sel!(applicationShouldTerminate:);
    let handler: Imp = unsafe { std::mem::transmute(application_should_terminate as *const ()) };
    unsafe {
        let added =
            objc2::ffi::class_addMethod(delegate_class, selector, handler, c"Q@:@".as_ptr());
        if !added.as_bool() {
            objc2::ffi::class_replaceMethod(delegate_class, selector, handler, c"Q@:@".as_ptr());
        }
        let handler: Imp = std::mem::transmute(system_will_terminate as *const ());
        objc2::ffi::class_addMethod(
            delegate_class,
            sel!(anlgSystemWillTerminate:),
            handler,
            c"v@:@".as_ptr(),
        );
    }
}

unsafe fn register_system_termination_observer(delegate: &objc2::runtime::AnyObject) {
    use objc2::sel;
    use objc2_app_kit::{NSWorkspace, NSWorkspaceWillPowerOffNotification};

    unsafe {
        NSWorkspace::sharedWorkspace()
            .notificationCenter()
            .addObserver_selector_name_object(
                delegate,
                sel!(anlgSystemWillTerminate:),
                Some(NSWorkspaceWillPowerOffNotification),
                None,
            );
    }
}

extern "C" fn system_will_terminate(
    _this: *mut objc2::runtime::AnyObject,
    _selector: objc2::runtime::Sel,
    _notification: *mut objc2::runtime::AnyObject,
) {
    anlg_intercept::set_force_quit();
    if let Some(app) = crate::APP_HANDLE.get() {
        app.exit(0);
    }
}

extern "C" fn application_should_terminate(
    _this: *mut objc2::runtime::AnyObject,
    _selector: objc2::runtime::Sel,
    _sender: *mut objc2::runtime::AnyObject,
) -> objc2_app_kit::NSApplicationTerminateReply {
    use objc2_app_kit::NSApplicationTerminateReply;

    if anlg_intercept::should_force_quit() {
        return NSApplicationTerminateReply::TerminateNow;
    }

    // The system Dock Quit action bypasses Tauri's custom Quit menu item.
    if let Some(app) = crate::APP_HANDLE.get() {
        tauri_plugin_tray::AnlgMenuItem::TrayQuit.handle(app);
    }
    NSApplicationTerminateReply::TerminateCancel
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2::{
        ClassType, msg_send,
        rc::Retained,
        runtime::{AnyObject, ClassBuilder},
    };
    use objc2_app_kit::NSApplicationTerminateReply;
    use objc2_foundation::NSObject;

    #[test]
    fn native_quit_keeps_app_running_until_full_termination_is_requested() {
        let class = ClassBuilder::new(c"AnarlogQuitTestDelegate", NSObject::class())
            .unwrap()
            .register();
        unsafe {
            register_termination_handler(class as *const _ as *mut _);
            let delegate: Retained<AnyObject> = msg_send![class, new];
            register_system_termination_observer(&delegate);
            let reply: NSApplicationTerminateReply = msg_send![&*delegate,
                applicationShouldTerminate: std::ptr::null_mut::<AnyObject>()];
            assert_eq!(reply, NSApplicationTerminateReply::TerminateCancel);

            objc2_app_kit::NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .postNotificationName_object(
                    objc2_app_kit::NSWorkspaceWillPowerOffNotification,
                    None,
                );
            let reply: NSApplicationTerminateReply = msg_send![&*delegate,
                applicationShouldTerminate: std::ptr::null_mut::<AnyObject>()];
            assert_eq!(reply, NSApplicationTerminateReply::TerminateNow);
            objc2_app_kit::NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .removeObserver(&delegate);
        }
    }
}
