use objc2::rc::Retained;
use objc2::{MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSControlStateValueOn, NSSwitch, NSView};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint};

use anlg_overlay_kit::macos::support::rect;

#[derive(Debug, Default)]
pub(crate) struct DecorativeSwitchIvars {}

define_class! {
    #[unsafe(super(NSSwitch))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnarlogPermissionAssistantDecorativeSwitch"]
    #[ivars = DecorativeSwitchIvars]
    pub(crate) struct DecorativeSwitch;

    impl DecorativeSwitch {
        // Non-interactive: let clicks fall through so the whole row drags.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
    }

    unsafe impl NSObjectProtocol for DecorativeSwitch {}
}

impl DecorativeSwitch {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DecorativeSwitchIvars::default());
        let this: Retained<Self> =
            unsafe { msg_send![super(this), initWithFrame: rect(0.0, 0.0, 0.0, 0.0)] };
        this.setState(NSControlStateValueOn);
        this
    }
}
