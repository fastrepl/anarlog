use std::cell::{Cell, RefCell};

use objc2::DefinedClass;
use objc2::Message;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSColor, NSCursor, NSView};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{MainThreadMarker, NSNumber, NSObjectProtocol, NSPoint, NSString};
use objc2_quartz_core::{
    CABasicAnimation, CALayer, CAMediaTiming, CAMediaTimingFunction, kCAMediaTimingFunctionEaseOut,
};

use anlg_overlay_kit::macos::support::{rect, with_effective_appearance};

use super::super::animation::{self, ANIM_GUIDE_VISIBILITY, FADE_DURATION};

#[derive(Debug, Default)]
pub(crate) struct GuideCursorViewIvars {
    pulse: RefCell<Option<Retained<CALayer>>>,
    visible: Cell<bool>,
}

define_class! {
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnarlogPermissionAssistantGuideCursorView"]
    #[ivars = GuideCursorViewIvars]
    pub(crate) struct GuideCursorView;

    impl GuideCursorView {
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            self.apply_pulse_style();
        }

        // Purely decorative hint; never intercept the drag.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
    }

    unsafe impl NSObjectProtocol for GuideCursorView {}
}

impl GuideCursorView {
    /// A faux macOS "grab" cursor with a pulsing ring behind it, floating over
    /// the item's top edge to nudge the user to move the mouse here and drag.
    /// Uses the real system open-hand cursor image so it matches the pointer
    /// they'll actually see. The pulse animation runs continuously; the drag
    /// source hides the whole view once the user starts interacting.
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let image = NSCursor::openHandCursor().image();
        let size = image.size();
        let frame = rect(0.0, 0.0, size.width, size.height);

        let this = Self::alloc(mtm).set_ivars(GuideCursorViewIvars {
            pulse: RefCell::new(None),
            visible: Cell::new(true),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setWantsLayer(true);

        let Some(root) = this.layer() else {
            return this;
        };
        let center = CGPoint::new(size.width / 2.0, size.height / 2.0);

        // Pulse ring, added first so it renders behind the cursor image.
        let pulse = CALayer::layer();
        let diameter = size.width.max(size.height) * 1.05;
        pulse.setBounds(CGRect::new(
            CGPoint::new(0.0, 0.0),
            CGSize::new(diameter, diameter),
        ));
        pulse.setPosition(center);
        pulse.setCornerRadius(diameter / 2.0);
        pulse.setBorderWidth(2.0);
        add_pulse_animation(&pulse);
        root.addSublayer(&pulse);
        *this.ivars().pulse.borrow_mut() = Some(pulse);
        this.apply_pulse_style();

        // Cursor image on top, with a soft shadow so it reads as hovering.
        let cursor = CALayer::layer();
        cursor.setBounds(CGRect::new(
            CGPoint::new(0.0, 0.0),
            CGSize::new(size.width, size.height),
        ));
        cursor.setPosition(center);
        cursor.setContentsScale(2.0);
        let contents: &AnyObject = image.as_ref();
        unsafe { cursor.setContents(Some(contents)) };
        cursor.setShadowColor(Some(&NSColor::blackColor().CGColor()));
        cursor.setShadowOpacity(0.3);
        cursor.setShadowRadius(2.5);
        cursor.setShadowOffset(CGSize::new(0.0, -1.5));
        root.addSublayer(&cursor);

        this
    }

    pub(crate) fn set_visible(&self, visible: bool) {
        if self.ivars().visible.get() == visible {
            return;
        }
        self.ivars().visible.set(visible);

        let Some(root) = self.layer() else {
            self.setHidden(!visible);
            return;
        };

        if visible {
            self.setHidden(false);
            animation::fade_layer(&root, 0.0, 1.0, FADE_DURATION, ANIM_GUIDE_VISIBILITY);
            return;
        }

        let from = root.opacity();
        let this = self.retain();
        animation::fade_layer_with_completion(
            &root,
            from,
            0.0,
            FADE_DURATION,
            ANIM_GUIDE_VISIBILITY,
            move || {
                this.setHidden(true);
                if let Some(layer) = this.layer() {
                    layer.setOpacity(1.0);
                }
            },
        );
    }

    fn apply_pulse_style(&self) {
        let pulse = self.ivars().pulse.borrow().clone();
        let Some(pulse) = pulse else {
            return;
        };
        with_effective_appearance(self, || {
            let accent = NSColor::controlAccentColor();
            pulse.setBorderColor(Some(&accent.colorWithAlphaComponent(0.9).CGColor()));
            pulse.setBackgroundColor(Some(&accent.colorWithAlphaComponent(0.16).CGColor()));
        });
    }
}

/// Adds an endlessly repeating expand-and-fade animation to `layer`, so it
/// reads as a gentle attention pulse.
fn add_pulse_animation(layer: &CALayer) {
    let scale =
        CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("transform.scale")));
    let scale_from = NSNumber::numberWithDouble(0.66);
    let scale_to = NSNumber::numberWithDouble(1.9);
    unsafe {
        scale.setFromValue(Some(scale_from.as_ref()));
        scale.setToValue(Some(scale_to.as_ref()));
    }
    scale.setDuration(1.5);
    scale.setRepeatCount(f32::INFINITY);
    scale.setTimingFunction(Some(&CAMediaTimingFunction::functionWithName(unsafe {
        kCAMediaTimingFunctionEaseOut
    })));
    layer.addAnimation_forKey(&scale, Some(&NSString::from_str("pulseScale")));

    let fade = CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
    let fade_from = NSNumber::numberWithDouble(0.55);
    let fade_to = NSNumber::numberWithDouble(0.0);
    unsafe {
        fade.setFromValue(Some(fade_from.as_ref()));
        fade.setToValue(Some(fade_to.as_ref()));
    }
    fade.setDuration(1.5);
    fade.setRepeatCount(f32::INFINITY);
    fade.setTimingFunction(Some(&CAMediaTimingFunction::functionWithName(unsafe {
        kCAMediaTimingFunctionEaseOut
    })));
    layer.addAnimation_forKey(&fade, Some(&NSString::from_str("pulseOpacity")));
}
