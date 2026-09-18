//! `main/left-sidebar-panel.ts` + `react-resizable-panels`' persisted layout:
//! the timeline sidebar is a share of the panel group's width (the window
//! minus the scaffold's `pl-1`), clamped to `LEFT_SIDEBAR_MIN_WIDTH_PX` /
//! `LEFT_SIDEBAR_MAX_WIDTH_PX`, and that share is what `autoSaveId`
//! persists under `react-resizable-panels:classic-main-sidebar`.

use serde_json::{Map, Value, json};

/// `LEFT_SIDEBAR_DEFAULT_WIDTH_PX` / `_MIN_` / `_MAX_`
pub const DEFAULT_WIDTH_PX: f32 = 200.0;
pub const MIN_WIDTH_PX: f32 = 200.0;
pub const MAX_WIDTH_PX: f32 = 360.0;
/// The scaffold's `pl-1` before the panel group.
pub const GROUP_INSET_PX: f32 = 4.0;
/// The `ResizableHandle` (`w-1`) between the panels.
pub const HANDLE_PX: f32 = 4.0;

/// `autoSaveId="classic-main-sidebar"`'s storage key, and the panel-id key of
/// the sidebar + content layout inside it.
pub const STORAGE_KEY: &str = "react-resizable-panels:classic-main-sidebar";
pub const LAYOUT_KEY: &str = "classic-main-content,classic-main-sidebar-left";

/// `createLeftSidebarPanelConstraints().defaultSize`: 200px of the group.
pub fn default_fraction(group_width: f32) -> f64 {
    (DEFAULT_WIDTH_PX as f64 / group_width.max(DEFAULT_WIDTH_PX) as f64).min(1.0)
}

/// The rendered width for a stored share of the group, the way the two flex
/// items split the group's free space (the group minus the handle): the
/// sidebar's `flex-grow` is the exact percentage (`leftSidebarPanelStyle`
/// overrides the library's), the content's is the library's
/// `toPrecision(3)` of its share, and `minWidth` / `maxWidth` (200px /
/// 360px) clamp the result.
pub fn width_for(fraction: f64, group_width: f32) -> f32 {
    let sidebar_grow = fraction * 100.0;
    let content_grow = to_precision_3(100.0 - sidebar_grow);
    let free = (group_width - HANDLE_PX).max(0.0) as f64;
    let width = (free * sidebar_grow / (sidebar_grow + content_grow).max(f64::EPSILON)) as f32;
    let max = MAX_WIDTH_PX.max(MIN_WIDTH_PX);
    width.clamp(MIN_WIDTH_PX, max)
}

/// `Number(value.toPrecision(3))`
fn to_precision_3(value: f64) -> f64 {
    if value == 0.0 {
        return 0.0;
    }
    let digits = value.abs().log10().floor() as i32 + 1;
    let decimals = (3 - digits).max(0) as usize;
    format!("{value:.decimals$}").parse().unwrap_or(value)
}

/// The share a dragged width leaves in the layout.
pub fn fraction_for(width: f32, group_width: f32) -> f64 {
    (width as f64 / group_width.max(1.0) as f64).clamp(0.0, 1.0)
}

/// The sidebar's share in a persisted `react-resizable-panels` document
/// (`layout: [sidebar%, content%]`).
pub fn read_layout_fraction(document: &str) -> Option<f64> {
    let value: Value = serde_json::from_str(document).ok()?;
    let percent = value.get(LAYOUT_KEY)?.get("layout")?.get(0)?.as_f64()?;
    (0.0..=100.0).contains(&percent).then_some(percent / 100.0)
}

/// The persisted document with the sidebar's share written the way the
/// library stores it: ten-decimal percentages for both panels, the other
/// entries left as they were.
pub fn with_layout_fraction(document: Option<&str>, fraction: f64) -> String {
    let mut root = document
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let sidebar = round_percent(fraction * 100.0);
    let content = round_percent(100.0 - sidebar);
    let mut entry = root
        .get(LAYOUT_KEY)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| {
            let mut entry = Map::new();
            entry.insert("expandToSizes".into(), json!({}));
            entry
        });
    entry.insert(
        "layout".into(),
        Value::Array(vec![percent_value(sidebar), percent_value(content)]),
    );
    root.insert(LAYOUT_KEY.into(), Value::Object(entry));
    Value::Object(root).to_string()
}

/// `Number(value.toFixed(10))`
fn round_percent(percent: f64) -> f64 {
    format!("{percent:.10}").parse().unwrap_or(percent)
}

/// A JavaScript number in JSON: no fraction digits for a whole value.
fn percent_value(percent: f64) -> Value {
    if percent.fract() == 0.0 {
        Value::from(percent as i64)
    } else {
        Value::from(percent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_the_share_within_the_pixel_constraints() {
        // 200px at the 1100px window the layout was saved in.
        let fraction = default_fraction(1096.0);
        assert!((fraction - 0.182_481_751_824_817_5).abs() < 1e-12);
        // The share alone would give 199.2px of the free space; `minWidth`
        // keeps it at 200.
        assert_eq!(width_for(fraction, 1096.0), 200.0);
        // Wider windows scale the share the way the inspector measured the
        // panel (253.890625px at a 1400px window, 226.5px at 1250px);
        // narrower ones clamp to the minimum.
        assert!((width_for(fraction, 1396.0) - 253.89).abs() < 0.01);
        assert!((width_for(fraction, 1246.0) - 226.53).abs() < 0.01);
        assert_eq!(width_for(fraction, 796.0), 200.0);
        assert_eq!(width_for(0.9, 1096.0), 360.0);
        assert!((fraction_for(254.0, 1396.0) * 1396.0 - 254.0).abs() < 1e-9);
        assert_eq!(to_precision_3(81.751_825), 81.8);
        assert_eq!(to_precision_3(5.123_4), 5.12);
        assert_eq!(to_precision_3(100.0), 100.0);
    }

    #[test]
    fn layout_documents_round_trip_the_library_shape() {
        let saved = r#"{"classic-main-content,classic-main-sidebar-left":{"expandToSizes":{},"layout":[18.2481751825,81.7518248175]}}"#;
        let fraction = read_layout_fraction(saved).unwrap();
        assert!((fraction - 0.182_481_751_825).abs() < 1e-12);
        assert_eq!(with_layout_fraction(Some(saved), fraction), saved);
        assert_eq!(with_layout_fraction(None, default_fraction(1096.0)), saved);
        // Other entries survive, the new one appended like a JS object's.
        let other = r#"{"other":{"layout":[50,50]}}"#;
        let written = with_layout_fraction(Some(other), 0.25);
        assert_eq!(
            written,
            r#"{"other":{"layout":[50,50]},"classic-main-content,classic-main-sidebar-left":{"expandToSizes":{},"layout":[25,75]}}"#
        );
        assert_eq!(read_layout_fraction(&written), Some(0.25));
        assert_eq!(read_layout_fraction("not json"), None);
        assert_eq!(read_layout_fraction(r#"{"x":1}"#), None);
    }
}
