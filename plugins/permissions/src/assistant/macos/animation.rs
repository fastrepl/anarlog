use objc2::Message;
use objc2_app_kit::{NSAnimatablePropertyContainer, NSPanel};

pub(crate) use anlg_overlay_kit::macos::animation::{
    APPEAR_DURATION, APPEAR_TRANSLATE_Y, DISMISS_DURATION, animate, animate_layer_background_color,
    animate_layer_scalar, animate_with_completion, fade_layer, fade_layer_with_completion,
    frame_delta, prepare_panel_for_reveal,
};

// Assistant overlay animation policy: durations, thresholds, and animation keys
// specific to the permission assistant chrome (not generic overlay mechanism).
pub(crate) const REPOSITION_DURATION: f64 = 0.12;
pub(crate) const HOVER_DURATION: f64 = 0.16;
pub(crate) const FADE_DURATION: f64 = 0.15;

pub(crate) const REPOSITION_THRESHOLD: f64 = 4.0;

pub(crate) const ANIM_GUIDE_VISIBILITY: &str = "guideVisibility";

const KEYPATH_TRANSLATE_Y: &str = "transform.translation.y";
const ANIM_REVEAL_TRANSLATE: &str = "permissionRevealTranslate";
const ANIM_DISMISS_TRANSLATE: &str = "permissionDismissTranslate";

pub(crate) fn reveal_panel(panel: &NSPanel) {
    let window = panel.retain();
    let content_layer = window.contentView().and_then(|view| view.layer());

    animate(APPEAR_DURATION, move || {
        window.animator().setAlphaValue(1.0);
    });

    if let Some(layer) = content_layer {
        animate_layer_scalar(
            &layer,
            KEYPATH_TRANSLATE_Y,
            APPEAR_TRANSLATE_Y,
            0.0,
            APPEAR_DURATION,
            ANIM_REVEAL_TRANSLATE,
        );
    }
}

pub(crate) fn dismiss_panel(panel: &NSPanel, completion: impl FnOnce() + 'static) {
    let window = panel.retain();
    let content_layer = window.contentView().and_then(|view| view.layer());

    animate_with_completion(
        DISMISS_DURATION,
        move || {
            window.animator().setAlphaValue(0.0);
        },
        completion,
    );

    if let Some(layer) = content_layer {
        animate_layer_scalar(
            &layer,
            KEYPATH_TRANSLATE_Y,
            0.0,
            APPEAR_TRANSLATE_Y,
            DISMISS_DURATION,
            ANIM_DISMISS_TRANSLATE,
        );
    }
}
