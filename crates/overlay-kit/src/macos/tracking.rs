use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSTrackingArea, NSTrackingAreaOptions};

use super::support::rect;

pub fn visible_hover_tracking_area(owner: &AnyObject) -> Retained<NSTrackingArea> {
    unsafe {
        NSTrackingArea::initWithRect_options_owner_userInfo(
            NSTrackingArea::alloc(),
            rect(0.0, 0.0, 0.0, 0.0),
            NSTrackingAreaOptions::MouseEnteredAndExited
                | NSTrackingAreaOptions::ActiveAlways
                | NSTrackingAreaOptions::InVisibleRect,
            Some(owner),
            None,
        )
    }
}
