//! `shared/main/chat-panels.tsx` (`MainChatPanels`) + `layout-widths.ts`:
//! the main body (sidebar + surface) and the right chat panel share the
//! panel group (the window minus the scaffold's `pl-1`) by percentage, with
//! CSS `min-width`s that win over the shares and let the group overflow (the
//! chat panel is clipped at the window's edge) when both cannot fit. The
//! share is what `autoSaveId="main-chat"` persists, and the note-surface
//! tabs guard the window width around it: the left sidebar collapses when the
//! note would drop under its minimum, and the window grows by the deficit
//! (restored when the panel closes).

use serde_json::{Map, Value, json};

/// `RIGHT_CHAT_PANEL_MIN_WIDTH_PX`
pub const RIGHT_PANEL_MIN_WIDTH_PX: f32 = 320.0;
/// `LEFT_SIDEBAR_MIN_WIDTH_PX`
pub const LEFT_SIDEBAR_MIN_WIDTH_PX: f32 = 200.0;
/// `NOTE_SURFACE_MIN_WIDTH_PX`, `STANDALONE_NOTE_SURFACE_MIN_WIDTH_PX`,
/// `AUTOMATIONS_SURFACE_MIN_WIDTH_PX`, `SETTINGS_SURFACE_MIN_WIDTH_PX`
pub const NOTE_SURFACE_MIN_WIDTH_PX: f32 = 500.0;
pub const STANDALONE_NOTE_SURFACE_MIN_WIDTH_PX: f32 = 420.0;
pub const AUTOMATIONS_SURFACE_MIN_WIDTH_PX: f32 = 600.0;
pub const SETTINGS_SURFACE_MIN_WIDTH_PX: f32 = 700.0;

/// The chat panel's `defaultSize={30} minSize={20} maxSize={50}`.
pub const DEFAULT_FRACTION: f64 = 0.3;
pub const MIN_FRACTION: f64 = 0.2;
pub const MAX_FRACTION: f64 = 0.5;

/// The `ResizableHandle` is `w-0`; the library still grabs it within its
/// fine-pointer hit-area margin.
pub const HANDLE_HIT_AREA_PX: f32 = 5.0;

/// `autoSaveId="main-chat"`'s storage key, and the key the library derives
/// from the two panels' constraints (sorted, comma-joined).
pub const STORAGE_KEY: &str = "react-resizable-panels:main-chat";
pub const LAYOUT_KEY: &str = r#"{"defaultSize":30,"maxSize":50,"minSize":20},{}"#;

/// Which surface the body panel is showing, for `getMainBodyMinWidth`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// `sessions` / `shared_sessions` / `shared_note_preview` / `empty`
    Note,
    /// The standalone note window (`note.$sessionId.tsx`).
    StandaloneNote,
    Automations,
    Settings,
    /// Contacts, calendar, templates, folders: no minimum.
    Other,
}

impl Surface {
    /// `usesNoteSurfaceMinWidth`: the tabs the window-width guard protects.
    pub fn uses_note_surface_min_width(self) -> bool {
        matches!(self, Surface::Note | Surface::StandaloneNote)
    }

    fn note_surface_min_width(self) -> Option<f32> {
        match self {
            Surface::Note => Some(NOTE_SURFACE_MIN_WIDTH_PX),
            Surface::StandaloneNote => Some(STANDALONE_NOTE_SURFACE_MIN_WIDTH_PX),
            _ => None,
        }
    }
}

/// `getMainBodyMinWidth` (+ `boundedMinWidthPx` for settings, whose
/// `min(<px>, 100%)` never exceeds the group).
pub fn body_min_width(surface: Surface, sidebar_expanded: bool, group_width: f32) -> Option<f32> {
    let sidebar = if sidebar_expanded {
        LEFT_SIDEBAR_MIN_WIDTH_PX
    } else {
        0.0
    };
    match surface {
        Surface::Automations => Some(AUTOMATIONS_SURFACE_MIN_WIDTH_PX + sidebar),
        Surface::Settings => Some((SETTINGS_SURFACE_MIN_WIDTH_PX + sidebar).min(group_width)),
        Surface::Note | Surface::StandaloneNote => {
            surface.note_surface_min_width().map(|min| min + sidebar)
        }
        Surface::Other => None,
    }
}

/// The two panels' rendered widths (body, chat) for the chat's share of the
/// group: flex items with `flex: <share> 1 0px`, so the free space splits by
/// share until a `min-width` clamps a panel, the other takes what is left,
/// and both minimums together overflow the group.
pub fn split(group_width: f32, fraction: f64, body_min: Option<f32>) -> (f32, f32) {
    let group = group_width.max(0.0);
    let fraction = clamp_fraction(fraction) as f32;
    let mut body = group * (1.0 - fraction);
    let mut chat = group * fraction;
    let body_min = body_min.unwrap_or(0.0);
    if body < body_min {
        body = body_min;
        chat = (group - body).max(0.0);
    }
    if chat < RIGHT_PANEL_MIN_WIDTH_PX {
        chat = RIGHT_PANEL_MIN_WIDTH_PX;
        body = (group - chat).max(body_min);
    }
    (body, chat)
}

/// `validatePanelGroupLayout` against the chat panel's `minSize` / `maxSize`.
pub fn clamp_fraction(fraction: f64) -> f64 {
    if fraction.is_finite() {
        fraction.clamp(MIN_FRACTION, MAX_FRACTION)
    } else {
        DEFAULT_FRACTION
    }
}

/// The share a drag leaves: the library moves the layout by the pointer's
/// delta as a percentage of the group, then validates it.
pub fn dragged_fraction(start_fraction: f64, delta_x: f32, group_width: f32) -> f64 {
    let delta = delta_x as f64 / group_width.max(1.0) as f64;
    clamp_fraction(start_fraction - delta)
}

/// The chat panel's share in a persisted `react-resizable-panels` document
/// (`layout: [body%, chat%]`).
pub fn read_layout_fraction(document: &str) -> Option<f64> {
    let value: Value = serde_json::from_str(document).ok()?;
    let percent = value.get(LAYOUT_KEY)?.get("layout")?.get(1)?.as_f64()?;
    (0.0..=100.0)
        .contains(&percent)
        .then_some(clamp_fraction(percent / 100.0))
}

/// The persisted document with the chat panel's share written the way the
/// library stores it: ten-decimal percentages for both panels, the other
/// entries (the single-panel `{}` layout among them) left as they were.
pub fn with_layout_fraction(document: Option<&str>, fraction: f64) -> String {
    let mut root = document
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let chat = round_percent(clamp_fraction(fraction) * 100.0);
    let body = round_percent(100.0 - chat);
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
        Value::Array(vec![percent_value(body), percent_value(chat)]),
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

/// `useNoteSurfaceWindowWidthGuard`'s inputs: whether the guard applies (a
/// note-surface tab) and which panels are open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PanelState {
    pub enabled: bool,
    pub left_open: bool,
    pub right_open: bool,
}

/// What the guard measures once the panels are laid out: the body panel's
/// visible width (the group minus the chat panel, at most the body itself),
/// the left sidebar's width and the chat panel's full width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    pub body_visible: f32,
    pub left_sidebar: f32,
    pub right_panel: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GuardStep {
    /// `restoreWidthExpansions`: undo every recorded expansion.
    RestoreExpansions,
    /// `collapseLeftPanel`: the note would drop under its minimum next to the
    /// chat panel.
    CollapseLeft,
    /// `windowExpandWidth(deficit, null, false, expandLeft, restoreOnClose)`
    ExpandWindow {
        deficit: f32,
        expand_left: bool,
        restore_on_close: bool,
    },
}

/// The guard's layout effect for a state change, in order.
pub fn guard_steps(
    previous: PanelState,
    current: PanelState,
    measured: Measured,
    note_min_width: f32,
) -> Vec<GuardStep> {
    let mut steps = Vec::new();
    let has_open_panel = current.enabled && (current.left_open || current.right_open);
    let right_just_closed = previous.right_open && !current.right_open;
    if right_just_closed || (current.enabled && !current.left_open && !current.right_open) {
        steps.push(GuardStep::RestoreExpansions);
    }
    if !has_open_panel {
        return steps;
    }
    let left_just_opened = current.left_open && (!previous.enabled || !previous.left_open);
    let right_just_opened = current.right_open && (!previous.enabled || !previous.right_open);
    if !left_just_opened && !right_just_opened {
        return steps;
    }
    let body_width = measured.body_visible;
    if body_width <= 0.0 {
        return steps;
    }
    let mut left_sidebar_width = if current.left_open {
        if measured.left_sidebar > 0.0 {
            measured.left_sidebar
        } else {
            LEFT_SIDEBAR_MIN_WIDTH_PX
        }
    } else {
        0.0
    };
    let right_panel_width = if current.right_open {
        measured.right_panel
    } else {
        0.0
    };
    if right_just_opened
        && current.left_open
        && left_sidebar_width > 0.0
        && body_width - left_sidebar_width < note_min_width
    {
        steps.push(GuardStep::CollapseLeft);
        left_sidebar_width = 0.0;
    }
    let required_body_width = note_min_width + left_sidebar_width;
    let required_total_width = required_body_width
        + if current.right_open {
            RIGHT_PANEL_MIN_WIDTH_PX
        } else {
            0.0
        };
    let visible_total_width = body_width + right_panel_width;
    let deficit = (required_body_width - body_width)
        .max(required_total_width - visible_total_width)
        .max(if current.right_open {
            RIGHT_PANEL_MIN_WIDTH_PX - right_panel_width
        } else {
            0.0
        })
        .ceil();
    if deficit > 0.0 {
        let expand_left = left_just_opened && !right_just_opened;
        steps.push(GuardStep::ExpandWindow {
            deficit,
            expand_left,
            restore_on_close: !expand_left,
        });
    }
    steps
}

/// `collapseLeftPanelIfNoteSurfaceWouldShrink`: on a resize that narrows the
/// visible body while the sidebar is open, collapse it once the note would be
/// under its minimum. `last` is the previous visible body width.
pub fn shrink_collapses_left(
    last: Option<f32>,
    body_visible: f32,
    left_sidebar: f32,
    note_min_width: f32,
) -> bool {
    let Some(last) = last else {
        return false;
    };
    if body_visible <= 0.0 || body_visible >= last {
        return false;
    }
    let left = if left_sidebar > 0.0 {
        left_sidebar
    } else {
        LEFT_SIDEBAR_MIN_WIDTH_PX
    };
    body_visible - left < note_min_width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_minimums_follow_the_surface_and_the_sidebar() {
        assert_eq!(body_min_width(Surface::Note, true, 1396.0), Some(700.0));
        assert_eq!(body_min_width(Surface::Note, false, 1396.0), Some(500.0));
        assert_eq!(
            body_min_width(Surface::StandaloneNote, false, 800.0),
            Some(420.0)
        );
        assert_eq!(
            body_min_width(Surface::Automations, true, 896.0),
            Some(800.0)
        );
        // `min(900px, 100%)`
        assert_eq!(body_min_width(Surface::Settings, true, 1396.0), Some(900.0));
        assert_eq!(body_min_width(Surface::Settings, true, 896.0), Some(896.0));
        assert_eq!(body_min_width(Surface::Other, true, 896.0), None);
    }

    #[test]
    fn the_split_matches_the_inspector() {
        // 1400px window, 30%: body 977.2 / chat 418.8.
        let (body, chat) = split(1396.0, 0.3, Some(700.0));
        assert!((body - 977.2).abs() < 0.01 && (chat - 418.8).abs() < 0.01);
        // Settings at 1200px: the body's 900px minimum leaves 296px, under the
        // chat's 320px minimum, so the group overflows by 24px.
        assert_eq!(split(1196.0, 0.3, Some(900.0)), (900.0, 320.0));
        // Settings at 900px: `min(900px, 100%)` fills the group and the chat
        // panel sits entirely past its edge.
        assert_eq!(split(896.0, 0.3, Some(896.0)), (896.0, 320.0));
        // Automations at 900px with the sidebar: 800 + 320.
        assert_eq!(split(896.0, 0.3, Some(800.0)), (800.0, 320.0));
        // No body minimum: the chat's minimum takes from the body.
        assert_eq!(split(1000.0, 0.3, None), (680.0, 320.0));
        // After the drag that left 37.2349570201% at 1400px.
        let (body, chat) = split(1396.0, 0.372_349_570_201, Some(900.0));
        assert!((body - 900.0).abs() < 0.01 && (chat - 496.0).abs() < 0.01);
    }

    #[test]
    fn drags_move_the_share_within_its_constraints() {
        // 101px leftwards at 1396px: 30% -> 37.23%.
        let fraction = dragged_fraction(0.3, -101.0, 1396.0);
        assert!((fraction - 0.372_349_570_2).abs() < 1e-9);
        assert_eq!(dragged_fraction(0.3, 400.0, 1000.0), MIN_FRACTION);
        assert_eq!(dragged_fraction(0.3, -400.0, 1000.0), MAX_FRACTION);
        assert_eq!(clamp_fraction(f64::NAN), DEFAULT_FRACTION);
    }

    #[test]
    fn layout_documents_round_trip_the_library_shape() {
        let saved = r#"{"{}":{"expandToSizes":{},"layout":[100]},"{\"defaultSize\":30,\"maxSize\":50,\"minSize\":20},{}":{"expandToSizes":{},"layout":[62.7650429799,37.2349570201]}}"#;
        let fraction = read_layout_fraction(saved).unwrap();
        assert!((fraction - 0.372_349_570_201).abs() < 1e-12);
        assert_eq!(with_layout_fraction(Some(saved), fraction), saved);
        assert_eq!(
            with_layout_fraction(None, DEFAULT_FRACTION),
            r#"{"{\"defaultSize\":30,\"maxSize\":50,\"minSize\":20},{}":{"expandToSizes":{},"layout":[70,30]}}"#
        );
        // Saved shares outside the constraints are validated on load.
        let wide =
            r#"{"{\"defaultSize\":30,\"maxSize\":50,\"minSize\":20},{}":{"layout":[20,80]}}"#;
        assert_eq!(read_layout_fraction(wide), Some(MAX_FRACTION));
        assert_eq!(read_layout_fraction("not json"), None);
        assert_eq!(read_layout_fraction(r#"{"x":1}"#), None);
    }

    fn state(enabled: bool, left_open: bool, right_open: bool) -> PanelState {
        PanelState {
            enabled,
            left_open,
            right_open,
        }
    }

    #[test]
    fn opening_the_chat_collapses_the_sidebar_and_grows_the_window() {
        // 900px window, sidebar 200, chat opens: the body keeps its 700px
        // minimum, 576px of it visible; 376px of note is under 500 so the
        // sidebar collapses, and 500 + 320 fits in 896.
        let steps = guard_steps(
            state(true, true, false),
            state(true, true, true),
            Measured {
                body_visible: 576.0,
                left_sidebar: 200.0,
                right_panel: 320.0,
            },
            500.0,
        );
        assert_eq!(steps, vec![GuardStep::CollapseLeft]);
        // 800px window: 476px visible, collapse, and 820 - 796 = 24px short.
        let steps = guard_steps(
            state(true, true, false),
            state(true, true, true),
            Measured {
                body_visible: 476.0,
                left_sidebar: 200.0,
                right_panel: 320.0,
            },
            500.0,
        );
        assert_eq!(
            steps,
            vec![
                GuardStep::CollapseLeft,
                GuardStep::ExpandWindow {
                    deficit: 24.0,
                    expand_left: false,
                    restore_on_close: true,
                },
            ]
        );
        // Plenty of room: nothing to do.
        let steps = guard_steps(
            state(true, true, false),
            state(true, true, true),
            Measured {
                body_visible: 977.0,
                left_sidebar: 254.0,
                right_panel: 419.0,
            },
            500.0,
        );
        assert!(steps.is_empty());
    }

    #[test]
    fn reopening_the_sidebar_grows_leftwards_without_a_restore() {
        // Chat open at 820px with the sidebar collapsed; the sidebar reopens:
        // the body's 700px minimum overflows, 500px visible, 200px short.
        let steps = guard_steps(
            state(true, false, true),
            state(true, true, true),
            Measured {
                body_visible: 500.0,
                left_sidebar: 200.0,
                right_panel: 320.0,
            },
            500.0,
        );
        assert_eq!(
            steps,
            vec![GuardStep::ExpandWindow {
                deficit: 200.0,
                expand_left: true,
                restore_on_close: false,
            }]
        );
    }

    #[test]
    fn closing_the_chat_or_leaving_both_closed_restores() {
        let measured = Measured {
            body_visible: 896.0,
            left_sidebar: 200.0,
            right_panel: 0.0,
        };
        assert_eq!(
            guard_steps(
                state(true, true, true),
                state(true, true, false),
                measured,
                500.0
            ),
            vec![GuardStep::RestoreExpansions]
        );
        assert_eq!(
            guard_steps(
                state(true, true, false),
                state(true, false, false),
                measured,
                500.0
            ),
            vec![GuardStep::RestoreExpansions]
        );
        // Off a note-surface tab nothing runs, and the chat closing there
        // still restores.
        assert!(
            guard_steps(
                state(true, true, true),
                state(false, true, true),
                measured,
                500.0
            )
            .is_empty()
        );
        assert_eq!(
            guard_steps(
                state(false, true, true),
                state(false, true, false),
                measured,
                500.0
            ),
            vec![GuardStep::RestoreExpansions]
        );
        // No change: nothing.
        assert!(
            guard_steps(
                state(true, true, true),
                state(true, true, true),
                measured,
                500.0
            )
            .is_empty()
        );
    }

    #[test]
    fn a_narrowing_resize_collapses_the_sidebar_under_the_minimum() {
        assert!(!shrink_collapses_left(None, 600.0, 200.0, 500.0));
        assert!(!shrink_collapses_left(Some(600.0), 700.0, 200.0, 500.0));
        assert!(!shrink_collapses_left(Some(800.0), 700.0, 200.0, 500.0));
        assert!(shrink_collapses_left(Some(800.0), 699.0, 200.0, 500.0));
        assert!(shrink_collapses_left(Some(800.0), 699.0, 0.0, 500.0));
    }
}
