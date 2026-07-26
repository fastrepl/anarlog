use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAppearance, NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, ns_string};

#[derive(Debug, Default)]
struct AppDelegateIvars {}

define_class! {
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "AppDelegate"]
    #[ivars = AppDelegateIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}
    unsafe impl NSApplicationDelegate for AppDelegate {}
}

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars::default());
        unsafe { msg_send![super(this), init] }
    }
}

pub fn run_app(f: impl FnOnce() + Send + 'static) {
    let mtm = MainThreadMarker::new().unwrap();

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    if let Some(appearance) = NSAppearance::appearanceNamed(ns_string!("NSAppearanceNameAqua")) {
        app.setAppearance(Some(&appearance));
    }

    let delegate = AppDelegate::new(mtm);
    app.setDelegate(Some(&ProtocolObject::from_ref(&*delegate)));

    std::thread::spawn(f);

    app.run();
}
