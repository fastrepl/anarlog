// Ported from Char crates/desktop-agent-automation/src/meeting_camera/matching.rs; see
// tools/camera-watermark/PROVENANCE.md.

//! Which Accessibility elements are a call's camera picker and its cameras, from their labels
//! alone, so the rules can be tested without a running call. Labels are an element's title,
//! description, help, DOM identifier and DOM classes, compared lowercase. English labels only.

use crate::meeting_automation::NativeMeetingApp;

/// The control beside the camera button that lists the cameras: Zoom's and Webex's "Video options"
/// caret, or Teams' camera options flyout button.
pub(crate) fn is_camera_picker(app: NativeMeetingApp, role: &str, labels: &[String]) -> bool {
    if !matches!(role, "AXButton" | "AXMenuButton" | "AXPopUpButton") {
        return false;
    }
    let labels = lower_all(labels);
    match app {
        NativeMeetingApp::Zoom | NativeMeetingApp::Webex => {
            labels.iter().any(|label| label == "video options")
        }
        NativeMeetingApp::Teams => labels.iter().any(|label| {
            label == "video-button-configure"
                || label == "video options"
                || label == "camera options"
                || label == "open video options"
                || label == "open camera options"
                || label == "more video options"
        }),
        NativeMeetingApp::Slack => false,
    }
}

/// An entry in the open picker that is the camera named `camera`: its label is exactly the name,
/// so "Anarlog Camera" never matches "Anarlog Camera (dev)", "Char Camera" or a similarly named
/// camera.
pub(crate) fn names_camera(camera: &str, labels: &[String]) -> bool {
    let camera = lower(camera);
    !camera.is_empty() && labels.iter().any(|label| lower(label) == camera)
}

/// An entry in a camera list: Zoom's menu rows, menu items or radios, or the list items of Teams'
/// camera options flyout, which Teams exposes as text with the `fui-ListItem` class.
pub(crate) fn is_camera_entry(role: &str, classes: &[String]) -> bool {
    match role {
        "AXRow" | "AXMenuItem" | "AXRadioButton" | "AXCheckBox" => true,
        "AXStaticText" => classes.iter().any(|class| class == "fui-ListItem"),
        _ => false,
    }
}

/// Webex's open camera list: a popover window (`UTMenuPopover`) described "Video options, N items".
pub(crate) fn is_webex_camera_menu(identifier: &str, description: &str) -> bool {
    identifier == "UTMenuPopover" && lower(description).starts_with("video options")
}

/// Whether a Webex camera row shows as the camera in use: its title reads "<name>, Checked, under
/// Camera group, 3 of 4"; the others leave out "Checked".
pub(crate) fn webex_entry_checked(camera: &str, labels: &[String]) -> bool {
    let checked = format!("{}, checked,", lower(camera));
    labels
        .iter()
        .any(|label| lower(label).starts_with(&checked))
}

/// What a meeting app draws behind the person. Either one cuts out everything but the person,
/// Anarlog Camera's badge included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundEffect {
    Blur,
    Image,
}

/// One choice in a meeting app's list of backgrounds: none, or an effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackgroundChoice {
    None,
    Effect(BackgroundEffect),
}

/// A choice in the Backgrounds part of Teams' camera flyout: menu items titled "No background
/// effect", "Standard blur", or "<name> background effect" for an image.
pub(crate) fn teams_background_choice(role: &str, labels: &[String]) -> Option<BackgroundChoice> {
    if role != "AXMenuItem" {
        return None;
    }
    let labels = lower_all(labels);
    if labels.iter().any(|label| label == "no background effect") {
        return Some(BackgroundChoice::None);
    }
    if labels.iter().any(|label| label.ends_with(" blur")) {
        return Some(BackgroundChoice::Effect(BackgroundEffect::Blur));
    }
    if labels
        .iter()
        .any(|label| label.ends_with(" background effect"))
    {
        return Some(BackgroundChoice::Effect(BackgroundEffect::Image));
    }
    None
}

/// What one look at Teams' background choices says, each choice with whether it is in use. Only
/// a single choice in use counts: none in use, or more than one, can't be told from Accessibility
/// and reads as `None`, so an unreadable list is never taken for no effect.
pub(crate) fn teams_chosen_background(
    choices: impl IntoIterator<Item = (BackgroundChoice, bool)>,
) -> Option<BackgroundChoice> {
    let mut chosen = choices.into_iter().filter(|(_, on)| *on).map(|(c, _)| c);
    let first = chosen.next()?;
    chosen.next().is_none().then_some(first)
}

/// The background Zoom's Settings shows as chosen, from one tile of its Backgrounds list. Tiles
/// read "None" or "Blur" (description) and "Backgrounds, San Francisco, 5 of 39" (value
/// description); the chosen one adds "Selected".
pub(crate) fn zoom_selected_background(
    identifier: &str,
    description: &str,
    value_description: &str,
) -> Option<BackgroundChoice> {
    if !matches!(
        identifier,
        "ZMVECollectionViewVBActionItem" | "ZMVirtualEffectsCollectionViewVBItem"
    ) {
        return None;
    }
    let selected = value_description
        .split(',')
        .any(|part| part.trim().eq_ignore_ascii_case("selected"));
    if !selected {
        return None;
    }
    Some(match lower(description).as_str() {
        "none" => BackgroundChoice::None,
        "blur" => BackgroundChoice::Effect(BackgroundEffect::Blur),
        _ => BackgroundChoice::Effect(BackgroundEffect::Image),
    })
}

/// The "Blur my background" switch in Zoom's camera menu.
pub(crate) fn is_zoom_blur_switch(role: &str, labels: &[String]) -> bool {
    role == "AXButton"
        && lower_all(labels)
            .iter()
            .any(|label| label == "blur my background")
}

/// Zoom's app menu item that opens Settings ("Settings...", or "Settings…").
pub(crate) fn is_zoom_settings_menu_item(title: &str) -> bool {
    matches!(lower(title).as_str(), "settings..." | "settings…")
}

/// The Video & effects page in Zoom Settings' category list, and its Backgrounds tab.
pub(crate) fn is_zoom_video_settings(label: &str) -> bool {
    lower(label).starts_with("video & effects")
}

/// Zoom reports its tabs' role as plain "Tab", so only the title counts.
pub(crate) fn is_zoom_backgrounds_tab(title: &str) -> bool {
    lower(title).starts_with("backgrounds,")
}

fn lower_all(labels: &[String]) -> Vec<String> {
    labels.iter().map(|label| lower(label)).collect()
}

fn lower(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn zoom_picker_is_the_video_options_caret() {
        assert!(is_camera_picker(
            NativeMeetingApp::Zoom,
            "AXButton",
            &labels(&["Video options"])
        ));
        // The camera button itself turns video on; it must never be pressed.
        assert!(!is_camera_picker(
            NativeMeetingApp::Zoom,
            "AXButton",
            &labels(&["Start video", "video"])
        ));
        assert!(!is_camera_picker(
            NativeMeetingApp::Zoom,
            "AXButton",
            &labels(&["Audio options"])
        ));
        assert!(!is_camera_picker(
            NativeMeetingApp::Zoom,
            "AXStaticText",
            &labels(&["Video options"])
        ));
    }

    #[test]
    fn teams_picker() {
        assert!(is_camera_picker(
            NativeMeetingApp::Teams,
            "AXButton",
            &labels(&["video-button-configure"])
        ));
        assert!(is_camera_picker(
            NativeMeetingApp::Teams,
            "AXButton",
            &labels(&["Open video options"])
        ));
        assert!(!is_camera_picker(
            NativeMeetingApp::Teams,
            "AXButton",
            &labels(&["Camera", "Turn camera on"])
        ));
        assert!(!is_camera_picker(
            NativeMeetingApp::Slack,
            "AXButton",
            &labels(&["Video options"])
        ));
    }

    /// Labels read live in a solo Webex (46.11) meeting on 2026-11-16.
    #[test]
    fn live_webex_labels() {
        assert!(is_camera_picker(
            NativeMeetingApp::Webex,
            "AXButton",
            &labels(&["Video options", "Video options"])
        ));
        // The camera button itself stops or starts video; it must never be pressed.
        assert!(!is_camera_picker(
            NativeMeetingApp::Webex,
            "AXButton",
            &labels(&["Stop video", " (⌃⇧V)", "callControl_video"])
        ));
        assert!(is_webex_camera_menu(
            "UTMenuPopover",
            "Video options, 7 items"
        ));
        assert!(!is_webex_camera_menu(
            "UTMenuPopover",
            "Audio options, 9 items"
        ));
        let row = labels(&[
            "Char Watermark, Checked, under Camera group, 3 of 4",
            "UTListTableCellView",
            "Char Watermark",
        ]);
        assert!(names_camera("Char Watermark", &row));
        assert!(webex_entry_checked("Char Watermark", &row));
        assert!(!webex_entry_checked(
            "Char Watermark",
            &labels(&[
                "Char Watermark, under Camera group, 3 of 4",
                "Char Watermark"
            ])
        ));
        // "Use last connected camera (…)" names a camera but isn't one.
        assert!(!names_camera(
            "Char Watermark",
            &labels(&["Use last connected camera (Char Watermark)"])
        ));
    }

    /// Labels read live in a solo Teams call's camera flyout on 2026-11-16.
    #[test]
    fn live_teams_background_labels() {
        assert_eq!(
            teams_background_choice("AXMenuItem", &labels(&["No background effect"])),
            Some(BackgroundChoice::None)
        );
        assert_eq!(
            teams_background_choice("AXMenuItem", &labels(&["Standard blur", "Standard blur"])),
            Some(BackgroundChoice::Effect(BackgroundEffect::Blur))
        );
        assert_eq!(
            teams_background_choice(
                "AXMenuItem",
                &labels(&["Starfield – Grav Jump background effect"])
            ),
            Some(BackgroundChoice::Effect(BackgroundEffect::Image))
        );
        // Soft focus and brightness are switches that keep the room, and the badge, in view.
        assert_eq!(
            teams_background_choice("AXCheckBox", &labels(&["Soft focus"])),
            None
        );
        assert_eq!(
            teams_background_choice("AXImage", &labels(&["Background effect Starfield"])),
            None
        );
        assert_eq!(
            teams_background_choice("AXMenuItem", &labels(&["MacBook Pro Camera"])),
            None
        );
    }

    /// Labels read live in Zoom 6 Settings → Video & effects → Backgrounds on 2026-11-16.
    #[test]
    fn live_zoom_background_labels() {
        assert_eq!(
            zoom_selected_background(
                "ZMVirtualEffectsCollectionViewVBItem",
                "",
                "Backgrounds, San Francisco, Selected, 5 of 39"
            ),
            Some(BackgroundChoice::Effect(BackgroundEffect::Image))
        );
        assert_eq!(
            zoom_selected_background(
                "ZMVirtualEffectsCollectionViewVBItem",
                "",
                "Backgrounds, Grass, 6 of 39"
            ),
            None
        );
        assert_eq!(
            zoom_selected_background(
                "ZMVECollectionViewVBActionItem",
                "None",
                "Selected, 1 of 39"
            ),
            Some(BackgroundChoice::None)
        );
        assert_eq!(
            zoom_selected_background("ZMVECollectionViewVBActionItem", "None", "1 of 39"),
            None
        );
        assert_eq!(
            zoom_selected_background(
                "ZMVECollectionViewVBActionItem",
                "Blur",
                "Selected, 2 of 39"
            ),
            Some(BackgroundChoice::Effect(BackgroundEffect::Blur))
        );
        // A selected tile elsewhere in Settings, such as a video filter, isn't a background.
        assert_eq!(
            zoom_selected_background("ZMVECollectionViewFilterItem", "None", "Selected, 1 of 9"),
            None
        );
        assert!(is_zoom_blur_switch(
            "AXButton",
            &labels(&["Blur my background"])
        ));
        assert!(!is_zoom_blur_switch(
            "AXButton",
            &labels(&["Auto-frame my video"])
        ));
        assert!(!is_zoom_blur_switch(
            "AXStaticText",
            &labels(&["Blur my background"])
        ));
        assert!(is_zoom_settings_menu_item("Settings..."));
        assert!(!is_zoom_settings_menu_item("Check for updates..."));
        assert!(is_zoom_video_settings("Video & effects"));
        assert!(!is_zoom_video_settings("Audio"));
        assert!(is_zoom_backgrounds_tab("Backgrounds, 3 of 6, Selected"));
        assert!(!is_zoom_backgrounds_tab("Video filters, 5 of 6"));
    }

    #[test]
    fn teams_reads_only_one_background_in_use() {
        use BackgroundChoice::{Effect, None};
        let blur = Effect(BackgroundEffect::Blur);
        assert_eq!(
            teams_chosen_background([(None, false), (blur, true)]),
            Some(blur)
        );
        assert_eq!(
            teams_chosen_background([(None, true), (blur, false)]),
            Some(None)
        );
        // Nothing reads as in use: unreadable, not "no effect".
        assert_eq!(
            teams_chosen_background([(None, false), (blur, false)]),
            Option::None
        );
        // Two in use at once: ambiguous.
        assert_eq!(
            teams_chosen_background([(None, true), (blur, true)]),
            Option::None
        );
        assert_eq!(teams_chosen_background([]), Option::None);
    }

    #[test]
    fn camera_entries() {
        assert!(is_camera_entry("AXRow", &[]));
        assert!(is_camera_entry("AXMenuItem", &[]));
        // Teams' flyout, as read live in a Teams call on 2026-11-16.
        assert!(is_camera_entry(
            "AXStaticText",
            &labels(&["fui-ListItem", "rn7e01x", "___a7qxlp0"])
        ));
        assert!(!is_camera_entry("AXStaticText", &labels(&["fui-Text"])));
        assert!(!is_camera_entry("AXGroup", &labels(&["fui-ListItem"])));
    }

    #[test]
    fn anarlog_camera_never_matches_char_camera() {
        let row = labels(&[
            "Anarlog Camera, Checked, under Camera group, 2 of 4",
            "UTListTableCellView",
            "Anarlog Camera",
        ]);
        assert!(names_camera("Anarlog Camera", &row));
        assert!(webex_entry_checked("Anarlog Camera", &row));
        for other in ["Char Camera", "Char Watermark", "Anarlog Camera (dev)"] {
            assert!(!names_camera("Anarlog Camera", &labels(&[other])));
        }
        assert!(!webex_entry_checked(
            "Anarlog Camera",
            &labels(&["Char Camera, Checked, under Camera group, 3 of 4"])
        ));
    }

    #[test]
    fn camera_names_match_exactly() {
        assert!(names_camera(
            "Char Watermark",
            &labels(&["", "char watermark "])
        ));
        assert!(!names_camera(
            "Char Watermark",
            &labels(&["Char Watermark (dev)"])
        ));
        assert!(!names_camera(
            "Char Watermark",
            &labels(&["MacBook Pro Camera"])
        ));
        assert!(!names_camera("", &labels(&[""])));
    }
}
