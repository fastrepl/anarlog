use super::{AxNode, MeetingPlatform};

pub(super) fn node_text(
    role: &Option<String>,
    title: &Option<String>,
    value: &Option<String>,
    description: &Option<String>,
    placeholder: &Option<String>,
) -> String {
    [role, title, value, description, placeholder]
        .into_iter()
        .filter_map(|v| v.as_deref())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(super) fn searchable_node_text(
    role: &Option<String>,
    title: &Option<String>,
    value: &Option<String>,
    description: &Option<String>,
    placeholder: &Option<String>,
    settable_value: bool,
) -> String {
    let hidden_value = None;
    node_text(
        role,
        title,
        if settable_value || is_text_input_role(role) {
            &hidden_value
        } else {
            value
        },
        description,
        placeholder,
    )
}

pub(super) fn is_text_input_role(role: &Option<String>) -> bool {
    matches!(
        role.as_deref(),
        Some("AXTextArea") | Some("AXTextField") | Some("AXSecureTextField")
    )
}

pub(super) fn node_labels(node: &AxNode) -> impl Iterator<Item = &str> {
    [
        node.title.as_deref(),
        node.placeholder.as_deref(),
        node.description.as_deref(),
        (!node.settable_value && !is_text_input_role(&node.role))
            .then_some(node.value.as_deref())
            .flatten(),
    ]
    .into_iter()
    .flatten()
}

pub(super) fn node_has_positive_bounds(node: &AxNode) -> bool {
    node.bounds
        .as_ref()
        .is_some_and(|bounds| bounds.width > 0.0 && bounds.height > 0.0)
}

pub(super) fn is_platform_meeting_control(platform: &MeetingPlatform, node: &AxNode) -> bool {
    if !matches!(
        node.role.as_deref(),
        Some("AXButton") | Some("AXMenuItem") | Some("AXPopUpButton")
    ) || node.enabled == Some(false)
    {
        return false;
    }

    node_labels(node).any(|label| {
        let label = label.trim().to_ascii_lowercase();
        match platform {
            MeetingPlatform::GoogleMeet => matches!(
                label.as_str(),
                "leave call"
                    | "turn on microphone"
                    | "turn off microphone"
                    | "turn on camera"
                    | "turn off camera"
                    | "present now"
            ),
            MeetingPlatform::MicrosoftTeams => matches!(
                label.as_str(),
                "hang up"
                    | "mute microphone"
                    | "unmute microphone"
                    | "turn camera on"
                    | "turn camera off"
            ),
            MeetingPlatform::Zoom => matches!(
                label.as_str(),
                "leave meeting" | "end meeting" | "mute my audio" | "unmute my audio"
            ),
            MeetingPlatform::Slack => label == "leave huddle",
            MeetingPlatform::Discord => label == "disconnect",
            MeetingPlatform::Webex => matches!(
                label.as_str(),
                "leave meeting"
                    | "end meeting"
                    | "leave meeting or end meeting for everyone"
                    | "mute me"
                    | "unmute me"
            ),
            MeetingPlatform::Unknown => false,
        }
    })
}

pub(super) fn is_platform_active_call_control(platform: &MeetingPlatform, node: &AxNode) -> bool {
    if !matches!(
        node.role.as_deref(),
        Some("AXButton") | Some("AXMenuItem") | Some("AXPopUpButton")
    ) || node.enabled == Some(false)
        || !node_has_positive_bounds(node)
    {
        return false;
    }

    node_labels(node).any(|label| {
        let label = label.trim().to_ascii_lowercase();
        match platform {
            MeetingPlatform::GoogleMeet => label == "leave call",
            MeetingPlatform::MicrosoftTeams => label == "hang up",
            MeetingPlatform::Zoom => {
                matches!(label.as_str(), "leave" | "leave meeting" | "end meeting")
            }
            MeetingPlatform::Slack => matches!(label.as_str(), "leave huddle" | "end huddle"),
            MeetingPlatform::Webex => matches!(
                label.as_str(),
                "leave meeting" | "end meeting" | "leave meeting or end meeting for everyone"
            ),
            MeetingPlatform::Discord | MeetingPlatform::Unknown => false,
        }
    })
}

pub(super) fn teams_has_active_call_evidence(nodes: &[AxNode]) -> bool {
    let has_leave = nodes.iter().any(|node| {
        matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
            && node.enabled != Some(false)
            && node_has_positive_bounds(node)
            && node_labels(node).any(|label| label.trim().eq_ignore_ascii_case("leave"))
    });
    let has_meeting_surface = nodes.iter().any(|node| {
        node_labels(node).any(|label| {
            matches!(
                label.trim().to_ascii_lowercase().as_str(),
                "meeting controls" | "calling controls" | "meeting chat"
            )
        })
    });

    has_leave && has_meeting_surface
}
