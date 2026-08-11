use std::cell::RefCell;

use block2::RcBlock;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSColor, NSImage,
    NSImageScaling, NSImageSymbolConfiguration, NSImageSymbolScale, NSImageView, NSView,
};
use objc2_foundation::{MainThreadMarker, NSArray, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{CALayer, kCACornerCurveContinuous};

pub fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

/// Rounds `layer`'s corners with the continuous ("squircle") curve used across
/// overlay chrome.
pub fn apply_continuous_corner(layer: &CALayer, radius: f64) {
    layer.setCornerRadius(radius);
    layer.setCornerCurve(unsafe { kCACornerCurveContinuous });
}

/// Background and optional border colors for a layer-backed rounded view.
pub struct FillStyle<'a> {
    pub background: &'a NSColor,
    pub border: Option<(&'a NSColor, f64)>,
}

/// Applies a continuous rounded fill (+ optional border) to a layer-backed
/// view, resolving colors under the view's effective appearance.
pub fn apply_rounded_fill(view: &NSView, radius: f64, style: FillStyle<'_>) {
    let Some(layer) = view.layer() else {
        return;
    };
    apply_continuous_corner(&layer, radius);
    layer.setMasksToBounds(true);
    with_effective_appearance(view, || {
        layer.setBackgroundColor(Some(&style.background.CGColor()));
        if let Some((border, width)) = style.border {
            layer.setBorderWidth(width);
            layer.setBorderColor(Some(&border.CGColor()));
        } else {
            layer.setBorderWidth(0.0);
            layer.setBorderColor(None);
        }
    });
}

pub fn srgb(rgb: (f64, f64, f64), alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(rgb.0, rgb.1, rgb.2, alpha)
}

pub fn is_dark_appearance(view: &NSView) -> bool {
    view.effectiveAppearance()
        .bestMatchFromAppearancesWithNames(&NSArray::from_slice(&[
            unsafe { NSAppearanceNameDarkAqua },
            unsafe { NSAppearanceNameAqua },
        ]))
        .is_some_and(|appearance| *appearance == *unsafe { NSAppearanceNameDarkAqua })
}

/// Resolves dynamic AppKit colors under `view`'s effective appearance.
///
/// Layer `CGColor`s are snapshots; re-resolve them inside
/// `viewDidChangeEffectiveAppearance` with this helper so they track live
/// Light/Dark Mode toggles.
pub fn with_effective_appearance<R>(view: &NSView, f: impl FnOnce() -> R) -> R {
    let appearance = view.effectiveAppearance();
    let callback = RefCell::new(Some(f));
    let out = RefCell::new(None);
    {
        let block = RcBlock::new(|| {
            if let Some(callback) = callback.borrow_mut().take() {
                *out.borrow_mut() = Some(callback());
            }
        });
        appearance.performAsCurrentDrawingAppearance(&block);
    }
    out.into_inner().expect("block runs synchronously")
}

pub fn symbol_image_view(
    mtm: MainThreadMarker,
    symbol_name: &str,
    point_size: f64,
    weight: f64,
    tint: &NSColor,
) -> Retained<NSImageView> {
    let name = NSString::from_str(symbol_name);
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&name, None)
        .unwrap_or_else(|| {
            NSImage::initWithSize(NSImage::alloc(), NSSize::new(point_size, point_size))
        });
    image.setTemplate(true);
    let config = NSImageSymbolConfiguration::configurationWithPointSize_weight_scale(
        point_size,
        weight,
        NSImageSymbolScale::Medium,
    );
    let configured = image
        .imageWithSymbolConfiguration(&config)
        .unwrap_or_else(|| image.clone());

    let view = NSImageView::imageViewWithImage(&configured, mtm);
    view.setImageScaling(NSImageScaling::ScaleProportionallyDown);
    view.setContentTintColor(Some(tint));
    view
}
