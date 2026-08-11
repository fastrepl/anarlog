use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSFont, NSLineBreakMode, NSTextField};
use objc2_foundation::{MainThreadMarker, NSRect, NSString};

pub fn framed_label(
    mtm: MainThreadMarker,
    text: &str,
    font: &NSFont,
    color: &NSColor,
    frame: NSRect,
    line_break_mode: NSLineBreakMode,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFrame(frame);
    label.setFont(Some(font));
    label.setTextColor(Some(color));
    label.setLineBreakMode(line_break_mode);
    label
}

pub fn sized_system_label(
    mtm: MainThreadMarker,
    text: &str,
    point_size: f64,
    weight: f64,
    color: &NSColor,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFont(Some(&NSFont::systemFontOfSize_weight(point_size, weight)));
    label.setTextColor(Some(color));
    label.sizeToFit();
    label
}
