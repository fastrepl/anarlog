use anlg_overlay_kit::macos::support::srgb;
use objc2::rc::Retained;
use objc2_app_kit::NSColor;

/// The app's warm accent (`hsl(27 87% 67%)`) in sRGB. Neutral chrome would
/// disappear against the System Settings pane the overlay sits on top of.
const ACCENT: (f64, f64, f64) = (0.957, 0.641, 0.383);

/// Row background for the drag target, tuned per appearance so it reads as a
/// drop zone in both without glaring on a dark desktop.
pub(crate) fn permission_drag_row_background(dark: bool, hovered: bool) -> Retained<NSColor> {
    let alpha = match (dark, hovered) {
        (true, true) => 0.34,
        (true, false) => 0.10,
        (false, true) => 0.55,
        (false, false) => 0.24,
    };
    srgb(ACCENT, alpha)
}
