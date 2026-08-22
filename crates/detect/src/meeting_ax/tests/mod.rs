use super::*;

fn node(index: usize, role: &str, title: &str, bounds: Option<AxRect>) -> AxNode {
    AxNode {
        index,
        tree_path: vec![index],
        element_hash: None,
        role: Some(role.to_string()),
        identifier: None,
        title: Some(title.to_string()),
        value: None,
        description: None,
        placeholder: None,
        enabled: Some(true),
        settable_value: false,
        bounds,
        text: node_text(
            &Some(role.to_string()),
            &Some(title.to_string()),
            &None,
            &None,
            &None,
        ),
        within_zoom_meeting_scope: false,
        within_zoom_chat_scope: false,
        within_slack_huddle_scope: false,
    }
}

fn fixture_node(index: usize, role: &str, title: &str, path: &[usize]) -> AxNode {
    let mut node = node(
        index,
        role,
        title,
        Some(AxRect {
            x: 10.0,
            y: 10.0,
            width: 120.0,
            height: 40.0,
        }),
    );
    node.tree_path = path.to_vec();
    node.element_hash = Some(0x1000 + index);
    node
}

fn fixture_composer(index: usize, title: &str, path: &[usize]) -> AxNode {
    let mut node = fixture_node(index, "AXTextArea", title, path);
    node.settable_value = true;
    node
}

fn ancestor(label: &str) -> AxAncestor {
    ancestor_at(label, &[0])
}

fn ancestor_at(label: &str, path: &[usize]) -> AxAncestor {
    AxAncestor {
        path: path.to_vec(),
        labels: vec![label.to_string()],
    }
}

fn zoom_message_node(index: usize, text: &str) -> AxNode {
    let mut node = node(index, "AXStaticText", text, None);
    node.within_zoom_meeting_scope = true;
    node.within_zoom_chat_scope = true;
    node
}

mod browser_detection;
mod capture_identity;
mod chat_scope;
mod inspection;
mod message_parsing;
mod participant_streams;
mod slack_huddle;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
#[test]
fn inspect_meeting_accessibility_is_empty_without_ax_backend() {
    assert!(inspect_meeting_accessibility().is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn inspect_meeting_accessibility_does_not_panic_without_meeting_apps() {
    let _ = inspect_meeting_accessibility();
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread")]
async fn meeting_ax_sync_entrypoints_do_not_panic_inside_tokio_runtime() {
    let _ = inspect_meeting_accessibility();
    let _ = capture_meeting_chat_messages(vec!["us.zoom.Zoom".to_string()]);
    let _ = send_meeting_chat_message("hello".to_string(), vec!["us.zoom.Zoom".to_string()]);
}
