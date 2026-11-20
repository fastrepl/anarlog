//! Which Accessibility elements are a call's chat, from their labels alone, so the rules can be
//! tested without a running call. Labels are the element's title, description, placeholder, help
//! and DOM classes, compared lowercase. English labels only for now.

use crate::meeting_automation::{NativeMeetingApp, matching::lower};

/// A recipient other than everyone in the call. A notice must never go to one person or to the
/// waiting room, so an input that names one is not the chat's input.
const NARROW_RECIPIENTS: &[&str] = &[
    "waiting room",
    "private message",
    "privately",
    "direct message",
];

/// The text input where a message goes to everyone in the call.
pub(crate) fn is_chat_input(app: NativeMeetingApp, role: &str, labels: &[String]) -> bool {
    if role != "AXTextArea" && role != "AXTextField" {
        return false;
    }
    let labels: Vec<String> = labels.iter().map(|label| lower(label)).collect();
    if labels.iter().any(|label| {
        NARROW_RECIPIENTS
            .iter()
            .any(|narrow| label.contains(narrow))
    }) {
        return false;
    }
    labels.iter().any(|label| match app {
        NativeMeetingApp::Zoom => {
            label.starts_with("message everyone") || label.starts_with("type message here")
        }
        NativeMeetingApp::Teams => {
            label.starts_with("type a message")
                || label.starts_with("type a new message")
                || label.starts_with("send a message")
        }
        NativeMeetingApp::Slack => {
            label == "ql-editor" || label.starts_with("message ") || label.starts_with("reply")
        }
        // "Write a message to everyone, Shift + Enter for a new line"; the Direct tab names one
        // person instead.
        NativeMeetingApp::Webex => label.starts_with("write a message to everyone,"),
    })
}

/// The button that shows the call's chat when it is closed.
pub(crate) fn is_open_chat_control(
    app: NativeMeetingApp,
    role: &str,
    identifier: &str,
    labels: &[String],
) -> bool {
    if role != "AXButton" && role != "AXCheckBox" && role != "AXRadioButton" {
        return false;
    }
    let labels: Vec<String> = labels.iter().map(|label| lower(label)).collect();
    match app {
        NativeMeetingApp::Zoom => {
            identifier == "chat"
                || labels
                    .iter()
                    .any(|label| matches!(label.as_str(), "chat" | "open chat" | "show chat"))
        }
        NativeMeetingApp::Teams => labels.iter().any(|label| {
            label == "chat" || label.starts_with("chat,") || label == "show conversation"
        }),
        NativeMeetingApp::Slack => labels.iter().any(|label| {
            label.contains("threadbutton") || label.contains("huddle thread") || label == "thread"
        }),
        // The same button reads "Hide chat panel" while the chat is open; that one closes it.
        NativeMeetingApp::Webex => {
            identifier == "callControl_message"
                && labels.iter().any(|label| label == "open chat panel")
        }
    }
}

/// A chat input's value that holds nothing the person typed: empty, or only its placeholder.
pub(crate) fn is_empty_draft(value: &str, placeholder: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value == placeholder.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn zoom_input_must_address_everyone() {
        let app = NativeMeetingApp::Zoom;
        assert!(is_chat_input(
            app,
            "AXTextArea",
            &labels(&["Message everyone, Shift+Command+E"])
        ));
        assert!(is_chat_input(
            app,
            "AXTextArea",
            &labels(&["", "Type message here..."])
        ));
        assert!(!is_chat_input(
            app,
            "AXTextArea",
            &labels(&["Message Alex (private message), Shift+Command+E"])
        ));
        assert!(!is_chat_input(
            app,
            "AXTextArea",
            &labels(&["Type message here...", "Message waiting room participants"])
        ));
        assert!(!is_chat_input(
            app,
            "AXButton",
            &labels(&["Message everyone"])
        ));
    }

    #[test]
    fn teams_and_slack_inputs() {
        assert!(is_chat_input(
            NativeMeetingApp::Teams,
            "AXTextArea",
            &labels(&["Type a message"])
        ));
        assert!(!is_chat_input(
            NativeMeetingApp::Teams,
            "AXTextArea",
            &labels(&["Search"])
        ));
        assert!(is_chat_input(
            NativeMeetingApp::Slack,
            "AXTextArea",
            &labels(&["ql-editor"])
        ));
        assert!(is_chat_input(
            NativeMeetingApp::Slack,
            "AXTextArea",
            &labels(&["Reply…"])
        ));
        assert!(!is_chat_input(
            NativeMeetingApp::Slack,
            "AXTextArea",
            &labels(&["Message Alex (direct message)"])
        ));
    }

    #[test]
    fn open_chat_controls() {
        assert!(is_open_chat_control(
            NativeMeetingApp::Zoom,
            "AXButton",
            "chat",
            &[]
        ));
        assert!(is_open_chat_control(
            NativeMeetingApp::Teams,
            "AXButton",
            "",
            &labels(&["Chat"])
        ));
        assert!(!is_open_chat_control(
            NativeMeetingApp::Teams,
            "AXButton",
            "",
            &labels(&["Chats"])
        ));
        assert!(is_open_chat_control(
            NativeMeetingApp::Slack,
            "AXButton",
            "",
            &labels(&["p-huddle_threadButton"])
        ));
    }

    /// Labels read live in Zoom 6 and Teams (personal) calls on 2026-11-16 (Char fixtures).
    #[test]
    fn live_call_labels() {
        assert!(is_chat_input(
            NativeMeetingApp::Zoom,
            "AXTextArea",
            &labels(&[
                "Message everyone",
                "Press tab for more options and to return to the main meeting window"
            ])
        ));
        assert!(is_open_chat_control(
            NativeMeetingApp::Zoom,
            "AXButton",
            "chat",
            &labels(&["Open chat panel", "Show chat (⇧⌘H)"])
        ));
        assert!(is_chat_input(
            NativeMeetingApp::Teams,
            "AXTextArea",
            &labels(&[
                "Type a message",
                "Type a message",
                "Type a message",
                "ck",
                "ck-content",
                "ck-editor__editable"
            ])
        ));
        assert!(is_open_chat_control(
            NativeMeetingApp::Teams,
            "AXButton",
            "",
            &labels(&["Chat", "Chat", "fui-Button"])
        ));
    }

    /// Labels read live in a solo Webex (46.11) meeting on 2026-11-16 (Char fixtures).
    #[test]
    fn live_webex_labels() {
        let app = NativeMeetingApp::Webex;
        assert!(is_chat_input(
            app,
            "AXTextArea",
            &labels(&["Write a message to everyone, Shift + Enter for a new line"])
        ));
        // A space in Webex's main window whose name starts with "Everyone" is not the call.
        assert!(!is_chat_input(
            app,
            "AXTextArea",
            &labels(&["Write a message to Everyone Team"])
        ));
        assert!(!is_chat_input(
            app,
            "AXTextArea",
            &labels(&["Write a message to Alex, Shift + Enter for a new line"])
        ));
        assert!(is_open_chat_control(
            app,
            "AXButton",
            "callControl_message",
            &labels(&["Open chat panel"])
        ));
        assert!(!is_open_chat_control(
            app,
            "AXButton",
            "callControl_message",
            &labels(&["Hide chat panel"])
        ));
    }

    #[test]
    fn drafts() {
        assert!(is_empty_draft("", "Type a message"));
        assert!(is_empty_draft("Type a message", "Type a message"));
        assert!(!is_empty_draft("half a sentence", "Type a message"));
    }
}
