mod new_note;

pub use new_note::DockNewNote;

pub trait DockMenuItem {
    const TITLE: &'static str;
    fn handle(app: &tauri::AppHandle<tauri::Wry>);
}

#[cfg(target_os = "macos")]
macro_rules! dock_menu_items {
    ($($variant:ident => $item:ty),* $(,)?) => {
        pub(crate) fn build_dock_menu(mtm: objc2_foundation::MainThreadMarker) -> objc2::rc::Retained<objc2_app_kit::NSMenu> {
            use objc2::MainThreadOnly;
            use objc2_app_kit::{NSMenu, NSMenuItem};
            use objc2_foundation::NSString;
            use $crate::menu_items::DockMenuItem;

            let menu = NSMenu::new(mtm);
            menu.setAutoenablesItems(false);

            $(
                {
                    let title = NSString::from_str(<$item as DockMenuItem>::TITLE);
                    let key_equivalent = NSString::from_str("");
                    let sel_name = std::ffi::CString::new(concat!(stringify!($variant), ":")).unwrap();
                    let sel = objc2::runtime::Sel::register(&sel_name);
                    let item = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            NSMenuItem::alloc(mtm),
                            &title,
                            Some(sel),
                            &key_equivalent,
                        )
                    };
                    item.setEnabled(true);
                    menu.addItem(&item);
                }
            )*

            menu
        }

        pub(crate) fn register_action_handlers(delegate_class: *mut objc2::runtime::AnyClass) {
            use $crate::menu_items::DockMenuItem;

            $(
                {
                    extern "C" fn handler(
                        _this: *mut objc2::runtime::AnyObject,
                        _sel: objc2::runtime::Sel,
                        _sender: *mut objc2::runtime::AnyObject,
                    ) {
                        if let Some(app) = $crate::APP_HANDLE.get() {
                            <$item as DockMenuItem>::handle(app);
                        }
                    }

                    let sel_name = std::ffi::CString::new(concat!(stringify!($variant), ":")).unwrap();
                    let sel = objc2::runtime::Sel::register(&sel_name);
                    let imp: objc2::runtime::Imp = unsafe { std::mem::transmute(handler as *const ()) };
                    let types = c"v@:@";

                    unsafe {
                        let added = objc2::ffi::class_addMethod(delegate_class, sel, imp, types.as_ptr());
                        if !added.as_bool() {
                            objc2::ffi::class_replaceMethod(delegate_class, sel, imp, types.as_ptr());
                        }
                    }
                }
            )*
        }
    };
}

#[cfg(target_os = "macos")]
dock_menu_items! {
    handleDockNewNote => DockNewNote,
}
