use objc2::rc::Retained;
use objc2::{MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSColor, NSView};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSRect};

use anlg_overlay_kit::macos::support::{apply_continuous_corner, with_effective_appearance};

#[derive(Debug, Default)]
pub(crate) struct ContentViewIvars {}

define_class! {
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnarlogPermissionAssistantContentView"]
    #[ivars = ContentViewIvars]
    pub(crate) struct ContentView;

    impl ContentView {
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            self.apply_layer_style();
        }
    }

    unsafe impl NSObjectProtocol for ContentView {}
}

impl ContentView {
    pub(crate) fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ContentViewIvars::default());
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setAutoresizesSubviews(true);
        this.setWantsLayer(true);
        this.apply_layer_style();
        this
    }

    pub(crate) fn apply_layer_style(&self) {
        let Some(layer) = self.layer() else {
            return;
        };
        apply_continuous_corner(&layer, 12.0);
        layer.setMasksToBounds(true);
        layer.setBorderWidth(0.5);
        with_effective_appearance(self, || {
            layer.setBackgroundColor(Some(&NSColor::windowBackgroundColor().CGColor()));
            layer.setBorderColor(Some(
                &NSColor::separatorColor()
                    .colorWithAlphaComponent(0.35)
                    .CGColor(),
            ));
        });
    }
}
