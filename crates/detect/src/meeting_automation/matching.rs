//! Which Accessibility elements only a call shows, from their labels alone, so the rules can be
//! tested without a running call. Shared by the chat notice and the camera switch.

use super::NativeMeetingApp;

/// The call's window: what the app shows only while a call is on.
pub(crate) fn is_call_window(app: NativeMeetingApp, identifier: &str, classes: &[String]) -> bool {
    match app {
        NativeMeetingApp::Zoom => identifier == "zm.meeting.window.main",
        // Teams and Slack are web views; their call windows are found by what they hold.
        // Webex shows its pre-join screen in the same kind of window as the call.
        NativeMeetingApp::Teams | NativeMeetingApp::Webex => false,
        NativeMeetingApp::Slack => classes
            .iter()
            .any(|class| class.starts_with("p-huddle_window")),
    }
}

/// A control inside a Teams window that only a call shows.
pub(crate) fn is_teams_call_marker(role: &str, labels: &[String]) -> bool {
    role == "AXToolbar"
        && labels.iter().any(|label| {
            matches!(
                lower(label).as_str(),
                "meeting controls" | "calling controls"
            )
        })
}

/// A control inside a Webex window that only a call shows: its End or Leave meeting button. The
/// pre-join screen, in the same `PopOutWindow`, has none.
pub(crate) fn is_webex_call_marker(role: &str, identifier: &str) -> bool {
    role == "AXButton" && identifier == "callControl_end"
}

pub(crate) fn lower(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn call_windows_and_markers() {
        assert!(is_call_window(
            NativeMeetingApp::Zoom,
            "zm.meeting.window.main",
            &[]
        ));
        assert!(!is_call_window(
            NativeMeetingApp::Zoom,
            "zm.home.window",
            &[]
        ));
        assert!(is_call_window(
            NativeMeetingApp::Slack,
            "",
            &labels(&["p-huddle_window__content"])
        ));
        assert!(!is_call_window(
            NativeMeetingApp::Webex,
            "PopOutWindow",
            &[]
        ));
        // Both the call window and Teams' floating compact view carry this toolbar.
        assert!(is_teams_call_marker(
            "AXToolbar",
            &labels(&["Meeting controls", "Meeting controls", "fui-Toolbar"])
        ));
        assert!(!is_teams_call_marker(
            "AXGroup",
            &labels(&["Meeting controls"])
        ));
        assert!(!is_teams_call_marker(
            "AXToolbar",
            &labels(&["Calling indicators"])
        ));
        assert!(is_webex_call_marker("AXButton", "callControl_end"));
        // The pre-join screen's Start meeting button doesn't mark a call.
        assert!(!is_webex_call_marker(
            "AXButton",
            "interstitial_joinMeeting_button"
        ));
    }
}
