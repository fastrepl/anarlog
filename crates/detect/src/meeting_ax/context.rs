use super::analysis::{
    chat_scope_label, is_explicit_chat_input, is_zoom_chat_scope_node,
    meeting_chat_surface_is_visible,
};
use super::{
    AxNode, BrowserMeetingRoot, MeetingPlatform, NativeMeetingRoot, browser_platform_from_url,
    is_platform_active_call_control, is_slack_huddle_composer, is_slack_thread_container_label,
    node_has_positive_bounds, node_labels, slack_huddle_context,
};

fn stable_capture_context_id(kind: &str, parts: &[String]) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for part in std::iter::once(kind).chain(parts.iter().map(String::as_str)) {
        for byte in part.as_bytes().iter().copied().chain(std::iter::once(0xff)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    format!("{kind}:{hash:016x}")
}

fn normalized_context_part(value: &str) -> String {
    value.trim().to_lowercase()
}

fn meeting_platform_context_kind(platform: &MeetingPlatform) -> &'static str {
    match platform {
        MeetingPlatform::Zoom => "zoom",
        MeetingPlatform::GoogleMeet => "google-meet",
        MeetingPlatform::MicrosoftTeams => "microsoft-teams",
        MeetingPlatform::Slack => "slack",
        MeetingPlatform::Discord => "discord",
        MeetingPlatform::Webex => "webex",
        MeetingPlatform::Unknown => "unknown",
    }
}

pub(super) fn path_is_ancestor(ancestor: &[usize], descendant: &[usize]) -> bool {
    ancestor.len() < descendant.len() && descendant.starts_with(ancestor)
}

fn common_tree_path(left: &[usize], right: &[usize]) -> Vec<usize> {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .map(|(part, _)| *part)
        .collect()
}

fn is_chat_scope_container(node: &AxNode) -> bool {
    if !matches!(
        node.role.as_deref(),
        Some("AXGroup")
            | Some("AXList")
            | Some("AXScrollArea")
            | Some("AXTable")
            | Some("AXSheet")
            | Some("AXLandmark")
    ) {
        return false;
    }

    let label = chat_scope_label(node);
    matches!(
        label.trim(),
        "chat"
            | "messages"
            | "meeting chat"
            | "in-call messages"
            | "conversation"
            | "message list"
            | "chat list"
            | "huddle chat"
            | "chat with everyone"
    ) || label.contains("meeting chat")
        || label.contains("in-call messages")
        || label.contains("chat messages")
        || label.contains("messages panel")
        || label.contains("huddle chat")
}

fn is_platform_chat_scope_container(platform: &MeetingPlatform, node: &AxNode) -> bool {
    if !is_chat_scope_container(node) {
        return false;
    }

    let label = chat_scope_label(node);
    match platform {
        MeetingPlatform::GoogleMeet => {
            label == "in-call messages" || label.contains("in-call messages")
        }
        MeetingPlatform::MicrosoftTeams => {
            label == "meeting chat" || label.contains("meeting chat")
        }
        MeetingPlatform::Zoom => {
            label == "chat" || label == "chat list" || label.contains("meeting chat")
        }
        MeetingPlatform::Slack => {
            label == "huddle chat"
                || label.contains("huddle chat")
                || label.contains("huddle thread")
                || label.contains("huddle messages")
        }
        MeetingPlatform::Webex => label == "chat with everyone" || label.contains("meeting chat"),
        MeetingPlatform::Discord | MeetingPlatform::Unknown => false,
    }
}

fn is_chat_message_list(node: &AxNode) -> bool {
    if !matches!(
        node.role.as_deref(),
        Some("AXGroup") | Some("AXList") | Some("AXScrollArea") | Some("AXTable")
    ) {
        return false;
    }

    let label = chat_scope_label(node);
    label == "conversation"
        || label == "message list"
        || label == "chat list"
        || label == "in-call messages"
        || label.contains("chat messages")
        || label.contains("meeting messages")
}

fn is_platform_chat_message_list(platform: &MeetingPlatform, node: &AxNode) -> bool {
    is_chat_message_list(node) && is_platform_chat_scope_container(platform, node)
}

fn is_platform_chat_composer(platform: &MeetingPlatform, node: &AxNode) -> bool {
    if !matches!(
        node.role.as_deref(),
        Some("AXTextArea") | Some("AXTextField")
    ) || node.enabled == Some(false)
        || !node.settable_value
        || !node_has_positive_bounds(node)
    {
        return false;
    }

    node_labels(node).any(|label| {
        let label = label.trim().to_ascii_lowercase();
        match platform {
            MeetingPlatform::GoogleMeet => label == "send a message",
            MeetingPlatform::MicrosoftTeams => matches!(
                label.as_str(),
                "type a message" | "type a new message" | "message everyone"
            ),
            MeetingPlatform::Zoom => {
                label == "message everyone" || label.starts_with("message to ")
            }
            MeetingPlatform::Slack => label.starts_with("message to "),
            MeetingPlatform::Webex => matches!(
                label.as_str(),
                "type a message" | "send a message" | "message everyone"
            ),
            MeetingPlatform::Discord | MeetingPlatform::Unknown => false,
        }
    })
}

pub(super) fn validated_chat_scope(
    platform: &MeetingPlatform,
    nodes: &[AxNode],
) -> Option<(Vec<usize>, Vec<usize>)> {
    if !matches!(
        platform,
        MeetingPlatform::Zoom
            | MeetingPlatform::GoogleMeet
            | MeetingPlatform::MicrosoftTeams
            | MeetingPlatform::Slack
            | MeetingPlatform::Webex
    ) || !nodes
        .iter()
        .any(|node| is_platform_active_call_control(platform, node))
    {
        return None;
    }

    if *platform == MeetingPlatform::Slack {
        return validated_slack_huddle_chat_scope(nodes);
    }

    let mut composers = nodes
        .iter()
        .filter(|node| is_platform_chat_composer(platform, node));
    let composer = composers.next()?;
    if composers.next().is_some() {
        return None;
    }

    let mut explicit_scopes = nodes
        .iter()
        .filter(|node| {
            path_is_ancestor(&node.tree_path, &composer.tree_path)
                && is_platform_chat_scope_container(platform, node)
        })
        .collect::<Vec<_>>();
    explicit_scopes.sort_by_key(|node| std::cmp::Reverse(node.tree_path.len()));
    if let Some(scope) = explicit_scopes.first() {
        return Some((scope.tree_path.clone(), composer.tree_path.clone()));
    }

    let mut message_lists = nodes
        .iter()
        .filter(|node| is_platform_chat_message_list(platform, node));
    let message_list = message_lists.next()?;
    if message_lists.next().is_some() {
        return None;
    }
    let scope_path = common_tree_path(&message_list.tree_path, &composer.tree_path);
    (!scope_path.is_empty()).then_some((scope_path, composer.tree_path.clone()))
}

fn validated_slack_huddle_chat_scope(nodes: &[AxNode]) -> Option<(Vec<usize>, Vec<usize>)> {
    let (_, channel) = slack_huddle_context(nodes)?;
    let mut composers = nodes
        .iter()
        .filter(|node| node_has_positive_bounds(node) && is_slack_huddle_composer(node, &channel));
    let composer = composers.next()?;
    if composers.next().is_some() {
        return None;
    }

    let mut thread_scopes = nodes
        .iter()
        .filter(|node| {
            path_is_ancestor(&node.tree_path, &composer.tree_path)
                && matches!(
                    node.role.as_deref(),
                    Some("AXGroup")
                        | Some("AXList")
                        | Some("AXScrollArea")
                        | Some("AXTable")
                        | Some("AXSheet")
                )
                && node_labels(node).any(|label| is_slack_thread_container_label(label, &channel))
        })
        .collect::<Vec<_>>();
    thread_scopes.sort_by_key(|node| std::cmp::Reverse(node.tree_path.len()));
    let scope = thread_scopes.first()?;
    Some((scope.tree_path.clone(), composer.tree_path.clone()))
}

fn canonical_browser_meeting_context(url: &str, platform: &MeetingPlatform) -> Option<String> {
    let mut url = url::Url::parse(url).ok()?;
    if browser_platform_from_url(Some(url.as_str())).as_ref() != Some(platform) {
        return None;
    }
    url.set_fragment(None);
    let _ = url.set_username("");
    let _ = url.set_password(None);
    let host = url.host_str()?.to_ascii_lowercase();
    url.set_host(Some(&host)).ok()?;
    Some(url.to_string())
}

pub(super) fn browser_capture_context_id(root: &BrowserMeetingRoot) -> Option<String> {
    let (scope_path, composer_path) = validated_chat_scope(&root.platform, &root.nodes)?;
    let canonical_url =
        canonical_browser_meeting_context(root.web_area_url.as_deref()?, &root.platform)?;
    let web_area_hash = root
        .nodes
        .iter()
        .find(|node| node.tree_path.is_empty())?
        .element_hash?;
    let scope_hash = root
        .nodes
        .iter()
        .find(|node| node.tree_path == scope_path)?
        .element_hash?;
    let composer_hash = root
        .nodes
        .iter()
        .find(|node| node.tree_path == composer_path)?
        .element_hash?;
    Some(stable_capture_context_id(
        meeting_platform_context_kind(&root.platform),
        &[
            canonical_url,
            format!("web-area:{web_area_hash:x}"),
            format!("scope:{scope_hash:x}"),
            format!("composer:{composer_hash:x}"),
        ],
    ))
}

pub(super) fn native_capture_context_id(
    platform: &MeetingPlatform,
    root: &NativeMeetingRoot,
) -> Option<String> {
    let (scope_path, composer_path) = validated_chat_scope(platform, &root.nodes)?;
    let window_hash = root
        .nodes
        .iter()
        .find(|node| node.tree_path.is_empty())?
        .element_hash?;
    let scope_hash = root
        .nodes
        .iter()
        .find(|node| node.tree_path == scope_path)?
        .element_hash?;
    let composer_hash = root
        .nodes
        .iter()
        .find(|node| node.tree_path == composer_path)?
        .element_hash?;
    Some(stable_capture_context_id(
        meeting_platform_context_kind(platform),
        &[
            format!("window:{window_hash:x}"),
            format!("scope:{scope_hash:x}"),
            format!("composer:{composer_hash:x}"),
        ],
    ))
}

pub(super) fn slack_capture_context_id(
    channel: &str,
    huddle_label: &str,
    window_hash: usize,
    composer_hash: usize,
) -> String {
    stable_capture_context_id(
        "slack",
        &[
            normalized_context_part(channel),
            normalized_context_part(huddle_label),
            format!("window:{window_hash:x}"),
            format!("composer:{composer_hash:x}"),
        ],
    )
}

fn zoom_context_id_from_parts(
    window_title: &str,
    window_hash: usize,
    chat_anchor_hash: usize,
) -> String {
    stable_capture_context_id(
        "zoom",
        &[
            normalized_context_part(window_title),
            format!("window:{window_hash:x}"),
            format!("chat:{chat_anchor_hash:x}"),
        ],
    )
}

pub(super) fn zoom_capture_context_id(root: &NativeMeetingRoot) -> Option<String> {
    let window_hash = root
        .nodes
        .iter()
        .find(|node| node.role.as_deref() == Some("AXWindow"))?
        .element_hash?;
    let chat_anchor_hash = root
        .nodes
        .iter()
        .find(|node| {
            node.within_zoom_meeting_scope
                && node.role.as_deref() == Some("AXTable")
                && is_zoom_chat_scope_node(node)
        })
        .and_then(|node| node.element_hash)
        .or_else(|| {
            root.nodes
                .iter()
                .find(|node| node.within_zoom_meeting_scope && is_explicit_chat_input(node))
                .and_then(|node| node.element_hash)
        })?;
    Some(zoom_context_id_from_parts(
        root.window_title.as_deref().unwrap_or_default(),
        window_hash,
        chat_anchor_hash,
    ))
}

pub(super) fn zoom_chat_surface_is_visible(nodes: &[AxNode]) -> bool {
    meeting_chat_surface_is_visible(&MeetingPlatform::Zoom, nodes)
}
