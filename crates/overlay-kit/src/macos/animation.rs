use std::cell::RefCell;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::Message;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSColor, NSPanel, NSWindow,
};
use objc2_foundation::{NSNumber, NSString};
use objc2_quartz_core::{
    CABasicAnimation, CALayer, CAMediaTiming, CAMediaTimingFunction, CATransaction,
    kCAMediaTimingFunctionEaseOut,
};

pub const APPEAR_DURATION: f64 = 0.32;
pub const DISMISS_DURATION: f64 = 0.18;

pub const APPEAR_SCALE: f64 = 0.92;
pub const APPEAR_TRANSLATE_Y: f64 = -12.0;

const KEYPATH_OPACITY: &str = "opacity";
pub const KEYPATH_SCALE: &str = "transform.scale";
const KEYPATH_TRANSLATE_Y: &str = "transform.translation.y";
const KEYPATH_BACKGROUND_COLOR: &str = "backgroundColor";

const ANIM_REVEAL_SCALE: &str = "revealScale";
const ANIM_REVEAL_TRANSLATE: &str = "revealTranslate";
const ANIM_DISMISS_SCALE: &str = "dismissScale";
const ANIM_DISMISS_TRANSLATE: &str = "dismissTranslate";

fn once_block(f: impl FnOnce() + 'static) -> RcBlock<dyn Fn()> {
    let cell = RefCell::new(Some(f));
    RcBlock::new(move || {
        if let Some(f) = cell.borrow_mut().take() {
            f();
        }
    })
}

fn once_context_block(
    duration: f64,
    body: impl FnOnce() + 'static,
) -> RcBlock<dyn Fn(NonNull<NSAnimationContext>)> {
    let body = RefCell::new(Some(body));
    RcBlock::new(move |ctx: NonNull<NSAnimationContext>| {
        let context = unsafe { ctx.as_ref() };
        context.setDuration(duration);
        let timing =
            CAMediaTimingFunction::functionWithName(unsafe { kCAMediaTimingFunctionEaseOut });
        context.setTimingFunction(Some(&timing));
        if let Some(body) = body.borrow_mut().take() {
            body();
        }
    })
}

fn basic_animation(key_path: &str, duration: f64) -> Retained<CABasicAnimation> {
    let animation = CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str(key_path)));
    animation.setDuration(duration);
    animation.setTimingFunction(Some(&CAMediaTimingFunction::functionWithName(unsafe {
        kCAMediaTimingFunctionEaseOut
    })));
    animation
}

fn set_number_values(animation: &CABasicAnimation, from: f64, to: f64) {
    let from_value = NSNumber::numberWithDouble(from);
    let to_value = NSNumber::numberWithDouble(to);
    unsafe {
        animation.setFromValue(Some(from_value.as_ref()));
        animation.setToValue(Some(to_value.as_ref()));
    }
}

fn set_float_values(animation: &CABasicAnimation, from: f32, to: f32) {
    let from_value = NSNumber::numberWithFloat(from);
    let to_value = NSNumber::numberWithFloat(to);
    unsafe {
        animation.setFromValue(Some(from_value.as_ref()));
        animation.setToValue(Some(to_value.as_ref()));
    }
}

pub fn animate(duration: f64, body: impl FnOnce() + 'static) {
    let block = once_context_block(duration, body);
    NSAnimationContext::runAnimationGroup(&block);
}

pub fn animate_with_completion(
    duration: f64,
    body: impl FnOnce() + 'static,
    completion: impl FnOnce() + 'static,
) {
    let block = once_context_block(duration, body);
    let completion_block = once_block(completion);
    NSAnimationContext::runAnimationGroup_completionHandler(&block, Some(&completion_block));
}

pub fn frame_delta(current: objc2_foundation::NSRect, target: objc2_foundation::NSRect) -> f64 {
    (current.origin.x - target.origin.x)
        .abs()
        .max((current.origin.y - target.origin.y).abs())
        .max((current.size.width - target.size.width).abs())
        .max((current.size.height - target.size.height).abs())
}

pub fn reveal_panel(panel: &NSPanel) {
    let window = panel.retain();
    let content_layer = window.contentView().and_then(|view| view.layer());

    animate(APPEAR_DURATION, move || {
        window.animator().setAlphaValue(1.0);
    });

    if let Some(layer) = content_layer {
        animate_layer_scalar(
            &layer,
            KEYPATH_SCALE,
            APPEAR_SCALE,
            1.0,
            APPEAR_DURATION,
            ANIM_REVEAL_SCALE,
        );
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

pub fn dismiss_panel(panel: &NSPanel, completion: impl FnOnce() + 'static) {
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
            KEYPATH_SCALE,
            1.0,
            APPEAR_SCALE,
            DISMISS_DURATION,
            ANIM_DISMISS_SCALE,
        );
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

pub fn fade_layer(layer: &CALayer, from: f32, to: f32, duration: f64, key: &str) {
    fade_layer_impl(layer, from, to, duration, key, None::<fn()>);
}

pub fn fade_layer_with_completion(
    layer: &CALayer,
    from: f32,
    to: f32,
    duration: f64,
    key: &str,
    completion: impl FnOnce() + 'static,
) {
    fade_layer_impl(layer, from, to, duration, key, Some(completion));
}

fn fade_layer_impl(
    layer: &CALayer,
    from: f32,
    to: f32,
    duration: f64,
    key: &str,
    completion: Option<impl FnOnce() + 'static>,
) {
    let animation = basic_animation(KEYPATH_OPACITY, duration);
    set_float_values(&animation, from, to);

    if let Some(completion) = completion {
        CATransaction::begin();
        CATransaction::setAnimationDuration(duration);
        unsafe {
            CATransaction::setCompletionBlock(Some(&once_block(completion)));
        }
        layer.addAnimation_forKey(&animation, Some(&NSString::from_str(key)));
        layer.setOpacity(to);
        CATransaction::commit();
    } else {
        layer.addAnimation_forKey(&animation, Some(&NSString::from_str(key)));
        layer.setOpacity(to);
    }
}

pub fn animate_layer_scalar(
    layer: &CALayer,
    key_path: &str,
    from: f64,
    to: f64,
    duration: f64,
    key: &str,
) {
    let animation = basic_animation(key_path, duration);
    set_number_values(&animation, from, to);
    layer.addAnimation_forKey(&animation, Some(&NSString::from_str(key)));
}

pub fn animate_layer_background_color(layer: &CALayer, to: &NSColor, duration: f64) {
    let to_color = to.CGColor();
    let animation = basic_animation(KEYPATH_BACKGROUND_COLOR, duration);
    if let Some(from) = layer.backgroundColor() {
        unsafe {
            animation.setFromValue(Some(from.as_ref()));
        }
    }
    unsafe {
        animation.setToValue(Some(to_color.as_ref()));
    }
    layer.addAnimation_forKey(
        &animation,
        Some(&NSString::from_str(KEYPATH_BACKGROUND_COLOR)),
    );
    layer.setBackgroundColor(Some(&to_color));
}

pub fn prepare_panel_for_reveal(panel: &NSPanel) {
    let window: &NSWindow = panel.as_ref();
    window.setAlphaValue(0.0);
}
