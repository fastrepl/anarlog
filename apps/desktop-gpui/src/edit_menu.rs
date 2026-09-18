//! The webview's editing context menu: a right-click inside an editable
//! (WebKit's contenteditable / input / textarea) focuses it and opens the
//! platform's Cut / Copy / Paste / Select All menu at the pointer. The
//! editables record the request here on their right mouse down; the
//! workspace's root handler, which runs after them, turns it into the menu.

use gpui::{App, FocusHandle, Pixels, Point};

#[derive(Default)]
pub struct Request(pub Option<(Point<Pixels>, FocusHandle)>);

impl gpui::Global for Request {}

/// Called from an editable's right mouse down after it took focus.
pub fn request(cx: &mut App, position: Point<Pixels>, focus: FocusHandle) {
    cx.default_global::<Request>().0 = Some((position, focus));
}

/// Takes the pending request, if a right-click landed in an editable.
pub fn take(cx: &mut App) -> Option<(Point<Pixels>, FocusHandle)> {
    cx.default_global::<Request>().0.take()
}
