#[cfg(target_os = "macos")]
use super::analysis::meeting_chat_surface_is_visible;
use super::analysis::{candidate_chat_target, is_explicit_chat_input, is_zoom_chat_scope_node};
use super::{
    AxNode, BrowserMeetingRoot, MeetingPlatform, NativeMeetingRoot, browser_platform_from_url,
    is_platform_active_call_control, is_slack_huddle_composer, is_slack_thread_container_label,
    node_has_positive_bounds, node_labels, slack_huddle_context, teams_has_active_call_evidence,
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

fn chat_scope_labels(node: &AxNode) -> impl Iterator<Item = String> + '_ {
    [
        node.title.as_deref(),
        node.value.as_deref(),
        node.description.as_deref(),
        node.placeholder.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_ascii_lowercase)
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

    chat_scope_labels(node).any(|label| {
        matches!(
            label.as_str(),
            "chat"
                | "messages"
                | "meeting chat"
                | "in-call messages"
                | "conversation"
                | "message list"
                | "chat message list"
                | "chat list"
                | "huddle chat"
                | "chat with everyone"
        ) || label.contains("meeting chat")
            || label.contains("in-call messages")
            || label.contains("chat messages")
            || label.contains("chat with everyone")
            || label.contains("messages panel")
            || label.contains("huddle chat")
    })
}

fn is_platform_chat_scope_container(platform: &MeetingPlatform, node: &AxNode) -> bool {
    if !is_chat_scope_container(node) {
        return false;
    }

    is_platform_chat_scope_label(platform, node)
}

fn is_platform_chat_scope_label(platform: &MeetingPlatform, node: &AxNode) -> bool {
    chat_scope_labels(node).any(|label| match platform {
        MeetingPlatform::GoogleMeet => {
            label == "in-call messages" || label.contains("in-call messages")
        }
        MeetingPlatform::MicrosoftTeams => {
            label == "meeting chat" || label.contains("meeting chat")
        }
        MeetingPlatform::Zoom => {
            label == "chat"
                || label == "chat list"
                || label == "chat message list"
                || label.contains("meeting chat")
        }
        MeetingPlatform::Slack => {
            label == "huddle chat"
                || label.contains("huddle chat")
                || label.contains("huddle thread")
                || label.contains("huddle messages")
        }
        MeetingPlatform::Webex => {
            label.contains("chat with everyone")
                || label.contains("meeting chat")
                || label.contains("chat tab list, everyone tab")
        }
        MeetingPlatform::Discord | MeetingPlatform::Unknown => false,
    })
}

fn is_chat_message_list(node: &AxNode) -> bool {
    if !matches!(
        node.role.as_deref(),
        Some("AXGroup") | Some("AXList") | Some("AXScrollArea") | Some("AXTable")
    ) {
        return false;
    }

    chat_scope_labels(node).any(|label| {
        label == "conversation"
            || label == "message list"
            || label == "chat message list"
            || label == "chat list"
            || label == "in-call messages"
            || label.contains("chat messages")
            || label.contains("meeting messages")
    })
}

fn is_platform_chat_message_list(platform: &MeetingPlatform, node: &AxNode) -> bool {
    (*platform == MeetingPlatform::Webex
        && matches!(
            node.role.as_deref(),
            Some("AXGroup") | Some("AXList") | Some("AXScrollArea") | Some("AXTable")
        )
        && chat_scope_labels(node).any(|label| label.contains("thread conversation history")))
        || (is_chat_message_list(node) && is_platform_chat_scope_container(platform, node))
}

pub(super) fn is_platform_chat_composer(platform: &MeetingPlatform, node: &AxNode) -> bool {
    is_platform_chat_composer_with_state(platform, node, true)
}

fn is_platform_chat_composer_with_state(
    platform: &MeetingPlatform,
    node: &AxNode,
    require_enabled: bool,
) -> bool {
    let is_webex_linux_capture_composer = !require_enabled
        && *platform == MeetingPlatform::Webex
        && node.role.as_deref() == Some("AXStaticText")
        && node.enabled != Some(false)
        && node.title.as_deref().is_some_and(|title| {
            let title = title.trim().to_ascii_lowercase();
            title.starts_with("write a message to everyone")
                && title.contains("press shift + enter for new line")
        });
    let is_editable_composer = matches!(
        node.role.as_deref(),
        Some("AXTextArea") | Some("AXTextField") | Some("AXComboBox")
    ) && (!require_enabled || node.enabled != Some(false))
        && node.settable_value;

    if (!is_editable_composer && !is_webex_linux_capture_composer)
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
                label == "message everyone"
                    || label.starts_with("message to ")
                    || label.starts_with("type message here")
            }
            MeetingPlatform::Slack => label.starts_with("message to "),
            MeetingPlatform::Webex => {
                matches!(
                    label.as_str(),
                    "type a message" | "send a message" | "message everyone"
                ) || label.starts_with("write a message to ")
            }
            MeetingPlatform::Discord | MeetingPlatform::Unknown => false,
        }
    })
}

pub(super) fn is_platform_send_button(
    platform: &MeetingPlatform,
    node: &AxNode,
    scope_path: &[usize],
) -> bool {
    if !matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
        || node.enabled == Some(false)
        || !(node.tree_path == scope_path || path_is_ancestor(scope_path, &node.tree_path))
    {
        return false;
    }

    node_labels(node).any(|label| {
        let label = label.trim().to_ascii_lowercase();
        match platform {
            MeetingPlatform::Slack => label == "send now",
            MeetingPlatform::Zoom
            | MeetingPlatform::GoogleMeet
            | MeetingPlatform::MicrosoftTeams
            | MeetingPlatform::Webex => {
                matches!(
                    label.as_str(),
                    "send" | "send now" | "send message" | "send a message"
                )
            }
            MeetingPlatform::Discord | MeetingPlatform::Unknown => false,
        }
    })
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn is_open_meeting_chat_control(node: &AxNode) -> bool {
    candidate_chat_target(node).is_some_and(|target| target.kind == "openChatControl")
        && node.enabled != Some(false)
}

pub(super) fn validated_chat_scope(
    platform: &MeetingPlatform,
    nodes: &[AxNode],
) -> Option<(Vec<usize>, Vec<usize>)> {
    validated_chat_scope_with_state(platform, nodes, true)
}

pub(super) fn validated_chat_capture_scope(
    platform: &MeetingPlatform,
    nodes: &[AxNode],
) -> Option<(Vec<usize>, Vec<usize>)> {
    validated_chat_scope_with_state(platform, nodes, false)
}

fn validated_chat_scope_with_state(
    platform: &MeetingPlatform,
    nodes: &[AxNode],
    require_enabled: bool,
) -> Option<(Vec<usize>, Vec<usize>)> {
    if !matches!(
        platform,
        MeetingPlatform::Zoom
            | MeetingPlatform::GoogleMeet
            | MeetingPlatform::MicrosoftTeams
            | MeetingPlatform::Slack
            | MeetingPlatform::Webex
    ) || !(nodes
        .iter()
        .any(|node| is_platform_active_call_control(platform, node))
        || (*platform == MeetingPlatform::MicrosoftTeams && teams_has_active_call_evidence(nodes)))
    {
        return None;
    }

    if *platform == MeetingPlatform::Slack {
        return validated_slack_huddle_chat_scope(nodes);
    }

    let mut composers = nodes
        .iter()
        .filter(|node| is_platform_chat_composer_with_state(platform, node, require_enabled));
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

    let mut labeled_scope_paths = nodes
        .iter()
        .filter(|node| is_platform_chat_scope_label(platform, node))
        .filter_map(|node| {
            let scope_path = common_tree_path(&node.tree_path, &composer.tree_path);
            let distance = node.tree_path.len() + composer.tree_path.len() - 2 * scope_path.len();
            let max_distance = match (require_enabled, platform) {
                (false, MeetingPlatform::MicrosoftTeams) => 14,
                (false, MeetingPlatform::Webex) => 10,
                _ => 6,
            };
            (!scope_path.is_empty() && distance <= max_distance).then_some(scope_path)
        })
        .collect::<Vec<_>>();
    labeled_scope_paths.sort();
    labeled_scope_paths.dedup();
    if let [scope_path] = labeled_scope_paths.as_slice() {
        return Some((scope_path.clone(), composer.tree_path.clone()));
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

fn browser_meeting_identity(root: &BrowserMeetingRoot) -> Option<String> {
    if let Some(url) = root.web_area_url.as_deref()
        && let Some(canonical) = canonical_browser_meeting_context(url, &root.platform)
    {
        return Some(canonical);
    }

    if root.platform == MeetingPlatform::GoogleMeet
        && let Some(code) = root
            .window_title
            .as_deref()
            .and_then(super::platform::google_meet_code_from_title)
    {
        return Some(format!("https://meet.google.com/{code}"));
    }

    #[cfg(any(test, target_os = "linux"))]
    {
        let title = root.window_title.as_deref()?;
        let title_platforms = super::browser_title_platform_signals(title);
        let has_active_call_control = root
            .nodes
            .iter()
            .any(|node| super::is_browser_active_call_control(&root.platform, node));
        if title_platforms.as_slice() == [root.platform.clone()] && has_active_call_control {
            return Some(format!(
                "atspi://{}/{}",
                meeting_platform_context_kind(&root.platform),
                normalized_context_part(title)
            ));
        }
    }

    None
}

pub(super) fn browser_capture_context_id(root: &BrowserMeetingRoot) -> Option<String> {
    let (scope_path, composer_path) = validated_chat_capture_scope(&root.platform, &root.nodes)?;
    let canonical_url = browser_meeting_identity(root)?;
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
    let (scope_path, composer_path) = validated_chat_capture_scope(platform, &root.nodes)?;
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
                && matches!(node.role.as_deref(), Some("AXTable") | Some("AXList"))
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

#[cfg(target_os = "macos")]
pub(super) fn zoom_chat_surface_is_visible(nodes: &[AxNode]) -> bool {
    meeting_chat_surface_is_visible(&MeetingPlatform::Zoom, nodes)
}
