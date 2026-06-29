#[cfg(target_os = "macos")]
use std::collections::HashSet;

#[cfg(target_os = "macos")]
use cidre::{arc, ax, cf, cg, ns};

#[cfg(target_os = "macos")]
const MEETING_APP_BUNDLES: &[&str] = &[
    "us.zoom.xos",
    "com.microsoft.teams2",
    "com.microsoft.teams",
    "com.tinyspeck.slackmacgap",
    "com.hnc.Discord",
    "com.google.Chrome",
    "com.microsoft.edgemac",
    "org.mozilla.firefox",
    "com.apple.Safari",
    "company.thebrowser.Browser",
];

#[cfg(target_os = "macos")]
const MAX_TREE_DEPTH: usize = 12;
#[cfg(target_os = "macos")]
const MAX_NODES: usize = 1800;
#[cfg(target_os = "macos")]
const MIN_VIDEO_AREA: f64 = 18_000.0;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MeetingPlatform {
    Zoom,
    GoogleMeet,
    MicrosoftTeams,
    Slack,
    Discord,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MeetingSurface {
    Native,
    Web,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq)]
pub struct AxRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingApp {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingParticipantStream {
    pub id: String,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub participant_name: Option<String>,
    pub label: Option<String>,
    pub bounds: Option<AxRect>,
    pub confidence: f32,
    pub is_active_speaker: bool,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingChatTarget {
    pub id: String,
    pub kind: String,
    pub label: Option<String>,
    pub bounds: Option<AxRect>,
    pub enabled: Option<bool>,
    pub settable: bool,
    pub confidence: f32,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingAccessibilityInspection {
    pub app: MeetingApp,
    pub pid: i32,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub accessibility_trusted: bool,
    pub window_title: Option<String>,
    pub participant_streams: Vec<MeetingParticipantStream>,
    pub latest_active_speakers: Vec<String>,
    pub chat_targets: Vec<MeetingChatTarget>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingChatSendResult {
    pub sent: bool,
    pub app: Option<MeetingApp>,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub input_label: Option<String>,
    pub send_action: Option<String>,
    pub warnings: Vec<String>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct AxNode {
    index: usize,
    role: Option<String>,
    title: Option<String>,
    value: Option<String>,
    description: Option<String>,
    placeholder: Option<String>,
    enabled: Option<bool>,
    settable_value: bool,
    bounds: Option<AxRect>,
    text: String,
}

#[cfg(target_os = "macos")]
struct AxChatElement {
    node: AxNode,
    element: arc::R<ax::UiElement>,
}

#[cfg(target_os = "macos")]
pub fn inspect_meeting_accessibility() -> Vec<MeetingAccessibilityInspection> {
    let accessibility_trusted = macos_accessibility_client::accessibility::application_is_trusted();
    running_meeting_apps()
        .into_iter()
        .map(|(app, pid)| inspect_app(app, pid, accessibility_trusted))
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn inspect_meeting_accessibility() -> Vec<MeetingAccessibilityInspection> {
    Vec::new()
}

#[cfg(target_os = "macos")]
pub fn send_meeting_chat_message(message: String) -> MeetingChatSendResult {
    let accessibility_trusted = macos_accessibility_client::accessibility::application_is_trusted();
    if !accessibility_trusted {
        return MeetingChatSendResult {
            sent: false,
            app: None,
            platform: MeetingPlatform::Unknown,
            surface: MeetingSurface::Unknown,
            input_label: None,
            send_action: None,
            warnings: vec!["macOS accessibility permission is not trusted".to_string()],
        };
    }

    for (app, pid) in running_meeting_apps() {
        let mut warnings = Vec::new();
        let ax_app = ax::UiElement::with_app_pid(pid);
        let _ = ax_app.set_messaging_timeout_secs(0.6);

        let mut nodes = Vec::new();
        collect_nodes(&ax_app, 0, &mut nodes, &mut warnings);
        let window_title = nodes.iter().find_map(|node| {
            (node.role.as_deref() == Some("AXWindow"))
                .then(|| node.title.clone())
                .flatten()
        });
        let platform = classify_platform(
            &app.id,
            window_title.as_deref(),
            &nodes,
            classify_bundle(&app.id),
        );
        let surface = classify_surface(&app.id, &platform);

        let mut chat_elements = Vec::new();
        collect_chat_elements(&ax_app, 0, &mut chat_elements);
        chat_elements
            .sort_by(|a, b| chat_element_score(&b.node).total_cmp(&chat_element_score(&a.node)));

        let input = chat_elements.iter().find(|item| {
            candidate_chat_target(&item.node).is_some_and(|target| target.kind == "input")
        });
        let Some(input) = input else {
            continue;
        };

        let label = input
            .node
            .title
            .clone()
            .or_else(|| input.node.placeholder.clone())
            .or_else(|| input.node.description.clone());

        let message_value = cf::String::from_str(&message);
        let mut input_element = input.element.retained();
        let _ = input_element.perform_action(ax::action::press());

        if let Err(error) = input_element.set_attr(ax::attr::value(), message_value.as_type_ref()) {
            warnings.push(format!("failed to set chat input value: {error:?}"));
            continue;
        }

        if input_element.perform_action(ax::action::confirm()).is_ok() {
            return MeetingChatSendResult {
                sent: true,
                app: Some(app),
                platform,
                surface,
                input_label: label,
                send_action: Some("confirm".to_string()),
                warnings,
            };
        }

        if let Some(button) = chat_elements.iter().find(|item| {
            candidate_chat_target(&item.node)
                .is_some_and(|target| target.kind == "sendButton" && target.enabled != Some(false))
        }) {
            if button.element.perform_action(ax::action::press()).is_ok() {
                return MeetingChatSendResult {
                    sent: true,
                    app: Some(app),
                    platform,
                    surface,
                    input_label: label,
                    send_action: Some("sendButton".to_string()),
                    warnings,
                };
            }
        }

        warnings.push("chat input was writable, but no send action succeeded".to_string());
        return MeetingChatSendResult {
            sent: false,
            app: Some(app),
            platform,
            surface,
            input_label: label,
            send_action: None,
            warnings,
        };
    }

    MeetingChatSendResult {
        sent: false,
        app: None,
        platform: MeetingPlatform::Unknown,
        surface: MeetingSurface::Unknown,
        input_label: None,
        send_action: None,
        warnings: vec!["no supported running meeting chat target found".to_string()],
    }
}

#[cfg(not(target_os = "macos"))]
pub fn send_meeting_chat_message(_message: String) -> MeetingChatSendResult {
    MeetingChatSendResult {
        sent: false,
        app: None,
        platform: MeetingPlatform::Unknown,
        surface: MeetingSurface::Unknown,
        input_label: None,
        send_action: None,
        warnings: vec!["meeting chat AX send is only available on macOS".to_string()],
    }
}

#[cfg(target_os = "macos")]
fn running_meeting_apps() -> Vec<(MeetingApp, i32)> {
    let mut seen = HashSet::new();
    let mut apps = Vec::new();

    for bundle_id in MEETING_APP_BUNDLES {
        let bundle = ns::String::with_str(bundle_id);
        let running = ns::RunningApp::with_bundle_id(&bundle);

        for app in running.iter() {
            let pid = app.pid();
            if !seen.insert(pid) {
                continue;
            }

            let name = app
                .localized_name()
                .map(|name| name.to_string())
                .unwrap_or_else(|| bundle_id.to_string());
            let id = app
                .bundle_id()
                .map(|id| id.to_string())
                .unwrap_or_else(|| bundle_id.to_string());

            apps.push((MeetingApp { id, name }, pid));
        }
    }

    apps
}

#[cfg(target_os = "macos")]
fn collect_chat_elements(element: &ax::UiElement, depth: usize, elements: &mut Vec<AxChatElement>) {
    if depth > MAX_TREE_DEPTH || elements.len() >= MAX_NODES {
        return;
    }

    let index = elements.len();
    let node = snapshot_node(element, index);
    if candidate_chat_target(&node).is_some() {
        elements.push(AxChatElement {
            node,
            element: element.retained(),
        });
    }

    let Ok(children) = element.children() else {
        return;
    };

    for child in children.iter() {
        collect_chat_elements(child, depth + 1, elements);
    }
}

#[cfg(target_os = "macos")]
fn chat_element_score(node: &AxNode) -> f32 {
    candidate_chat_target(node)
        .map(|target| target.confidence)
        .unwrap_or(0.0)
}

#[cfg(target_os = "macos")]
fn inspect_app(
    app: MeetingApp,
    pid: i32,
    accessibility_trusted: bool,
) -> MeetingAccessibilityInspection {
    let mut warnings = Vec::new();
    let bundle_platform = classify_bundle(&app.id);
    let mut window_title = None;
    let mut nodes = Vec::new();

    if accessibility_trusted {
        let ax_app = ax::UiElement::with_app_pid(pid);
        let _ = ax_app.set_messaging_timeout_secs(0.6);
        collect_nodes(&ax_app, 0, &mut nodes, &mut warnings);
        window_title = nodes.iter().find_map(|node| {
            (node.role.as_deref() == Some("AXWindow"))
                .then(|| node.title.clone())
                .flatten()
        });
    } else {
        warnings.push("macOS accessibility permission is not trusted".to_string());
    }

    let platform = classify_platform(&app.id, window_title.as_deref(), &nodes, bundle_platform);
    let surface = classify_surface(&app.id, &platform);
    let participant_streams = find_participant_streams(&platform, &surface, &nodes);
    let latest_active_speakers = participant_streams
        .iter()
        .filter(|stream| stream.is_active_speaker)
        .filter_map(|stream| {
            stream
                .participant_name
                .clone()
                .or_else(|| stream.label.clone())
        })
        .collect();
    let chat_targets = find_chat_targets(&nodes);

    MeetingAccessibilityInspection {
        app,
        pid,
        platform,
        surface,
        accessibility_trusted,
        window_title,
        participant_streams,
        latest_active_speakers,
        chat_targets,
        warnings,
    }
}

#[cfg(target_os = "macos")]
fn collect_nodes(
    element: &ax::UiElement,
    depth: usize,
    nodes: &mut Vec<AxNode>,
    warnings: &mut Vec<String>,
) {
    if depth > MAX_TREE_DEPTH || nodes.len() >= MAX_NODES {
        return;
    }

    let index = nodes.len();
    nodes.push(snapshot_node(element, index));

    let children = match element.children() {
        Ok(children) => children,
        Err(_) => return,
    };

    for child in children.iter() {
        if nodes.len() >= MAX_NODES {
            warnings.push(format!("AX tree truncated at {MAX_NODES} nodes"));
            return;
        }

        collect_nodes(child, depth + 1, nodes, warnings);
    }
}

#[cfg(target_os = "macos")]
fn snapshot_node(element: &ax::UiElement, index: usize) -> AxNode {
    let role = element.role().ok().map(|role| role.to_string());
    let title = string_attr(element, ax::attr::title());
    let value = string_attr(element, ax::attr::value());
    let description = string_attr(element, ax::attr::desc());
    let placeholder = string_attr(element, ax::attr::placeholder_value());
    let enabled = element.is_enabled().ok().map(|enabled| enabled.value());
    let settable_value = element.is_settable(ax::attr::value()).unwrap_or(false);
    let bounds = element
        .frame()
        .ok()
        .and_then(|frame| frame.cg_rect())
        .or_else(|| rect_from_position_and_size(element))
        .map(AxRect::from);
    let text = node_text(&role, &title, &value, &description, &placeholder);

    AxNode {
        index,
        role,
        title,
        value,
        description,
        placeholder,
        enabled,
        settable_value,
        bounds,
        text,
    }
}

#[cfg(target_os = "macos")]
fn string_attr(element: &ax::UiElement, attr: &ax::Attr) -> Option<String> {
    let value = element.attr_value(attr).ok()?;
    value.try_as_string().map(|s| s.to_string())
}

#[cfg(target_os = "macos")]
fn rect_from_position_and_size(element: &ax::UiElement) -> Option<cg::Rect> {
    let position = element.pos().ok()?.cg_point()?;
    let size = element.size().ok()?.cg_size()?;
    Some(cg::Rect {
        origin: position,
        size,
    })
}

#[cfg(target_os = "macos")]
fn node_text(
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

#[cfg(target_os = "macos")]
fn classify_bundle(bundle_id: &str) -> MeetingPlatform {
    match bundle_id {
        "us.zoom.xos" => MeetingPlatform::Zoom,
        "com.microsoft.teams2" | "com.microsoft.teams" => MeetingPlatform::MicrosoftTeams,
        "com.tinyspeck.slackmacgap" => MeetingPlatform::Slack,
        "com.hnc.Discord" => MeetingPlatform::Discord,
        _ => MeetingPlatform::Unknown,
    }
}

#[cfg(target_os = "macos")]
fn classify_platform(
    bundle_id: &str,
    window_title: Option<&str>,
    nodes: &[AxNode],
    bundle_platform: MeetingPlatform,
) -> MeetingPlatform {
    if bundle_platform != MeetingPlatform::Unknown {
        return bundle_platform;
    }

    let title = window_title.unwrap_or_default().to_lowercase();
    let has_text = |needle: &str| {
        title.contains(needle) || nodes.iter().any(|node| node.text.contains(needle))
    };

    if has_text("meet.google.com") || has_text("google meet") {
        MeetingPlatform::GoogleMeet
    } else if has_text("teams.microsoft.com") || has_text("microsoft teams") {
        MeetingPlatform::MicrosoftTeams
    } else if has_text("zoom.us") || has_text("zoom meeting") {
        MeetingPlatform::Zoom
    } else if has_text("slack.com") || has_text("huddle") {
        MeetingPlatform::Slack
    } else if has_text("discord.com") || has_text("voice connected") {
        MeetingPlatform::Discord
    } else if is_browser_bundle(bundle_id) {
        MeetingPlatform::Unknown
    } else {
        bundle_platform
    }
}

#[cfg(target_os = "macos")]
fn classify_surface(bundle_id: &str, platform: &MeetingPlatform) -> MeetingSurface {
    if is_browser_bundle(bundle_id) {
        MeetingSurface::Web
    } else if *platform == MeetingPlatform::Unknown {
        MeetingSurface::Unknown
    } else {
        MeetingSurface::Native
    }
}

#[cfg(target_os = "macos")]
fn is_browser_bundle(bundle_id: &str) -> bool {
    matches!(
        bundle_id,
        "com.google.Chrome"
            | "com.microsoft.edgemac"
            | "org.mozilla.firefox"
            | "com.apple.Safari"
            | "company.thebrowser.Browser"
    )
}

#[cfg(target_os = "macos")]
fn find_participant_streams(
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
    nodes: &[AxNode],
) -> Vec<MeetingParticipantStream> {
    let mut streams = nodes
        .iter()
        .filter_map(|node| candidate_stream(platform, surface, node))
        .collect::<Vec<_>>();

    streams.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    streams.truncate(24);
    streams
}

#[cfg(target_os = "macos")]
fn candidate_stream(
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
    node: &AxNode,
) -> Option<MeetingParticipantStream> {
    let role = node.role.as_deref().unwrap_or_default();
    let text = node.text.as_str();
    let area = node
        .bounds
        .as_ref()
        .map(|r| r.width * r.height)
        .unwrap_or(0.0);
    let mut signals = Vec::new();
    let mut confidence = 0.0;

    if role == "AXImage" {
        confidence += 0.25;
        signals.push("image-role".to_string());
    }
    if role == "AXGroup" && area >= MIN_VIDEO_AREA {
        confidence += 0.15;
        signals.push("large-group".to_string());
    }
    if text.contains("video render") || text.contains("video tile") || text.contains("video") {
        confidence += 0.45;
        signals.push("video-label".to_string());
    }
    if text.contains("profile") {
        confidence += 0.2;
        signals.push("profile-child".to_string());
    }
    if text.contains("speaking")
        || text.contains("active speaker")
        || text.contains("computer audio unmuted")
    {
        confidence += 0.25;
        signals.push("speaker-state-label".to_string());
    }
    if area >= MIN_VIDEO_AREA {
        confidence += 0.15;
        signals.push("video-sized-bounds".to_string());
    }

    if confidence < 0.35 {
        return None;
    }

    let label = node
        .title
        .clone()
        .or_else(|| node.description.clone())
        .or_else(|| node.value.clone());
    let participant_name = participant_name_from_label(label.as_deref());
    let is_active_speaker = signals.iter().any(|signal| signal == "speaker-state-label");

    Some(MeetingParticipantStream {
        id: format!("ax-node-{}", node.index),
        platform: platform.clone(),
        surface: surface.clone(),
        participant_name,
        label,
        bounds: node.bounds.clone(),
        confidence,
        is_active_speaker,
        signals,
    })
}

#[cfg(target_os = "macos")]
fn participant_name_from_label(label: Option<&str>) -> Option<String> {
    let label = label?.trim();

    if let Some(name) = label.strip_prefix("Video render ") {
        return Some(
            name.trim_end_matches(", Computer audio unmuted")
                .to_string(),
        );
    }

    if let Some(name) = label.strip_prefix("View ") {
        return Some(name.trim_end_matches("'s profile").to_string());
    }

    if label.len() <= 80 && !label.eq_ignore_ascii_case("video") {
        Some(label.to_string())
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn find_chat_targets(nodes: &[AxNode]) -> Vec<MeetingChatTarget> {
    let mut targets = nodes
        .iter()
        .filter_map(candidate_chat_target)
        .collect::<Vec<_>>();

    targets.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    targets.truncate(16);
    targets
}

#[cfg(target_os = "macos")]
fn candidate_chat_target(node: &AxNode) -> Option<MeetingChatTarget> {
    let role = node.role.as_deref().unwrap_or_default();
    let text = node.text.as_str();
    let mut confidence = 0.0;
    let mut signals = Vec::new();
    let mut kind = "unknown";

    if role == "AXTextArea" || role == "AXTextField" {
        confidence += 0.25;
        signals.push("text-input-role".to_string());
        kind = "input";
    }
    if text.contains("send a message")
        || text.contains("message everyone")
        || text.contains("type a message")
        || text.contains("chat")
    {
        confidence += 0.4;
        signals.push("chat-label".to_string());
    }
    if text.contains("send") && role == "AXButton" {
        confidence += 0.35;
        signals.push("send-button".to_string());
        kind = "sendButton";
    }
    if text.contains("conversation") || text.contains("message list") {
        confidence += 0.25;
        signals.push("message-list-label".to_string());
        kind = "messageList";
    }
    if node.settable_value {
        confidence += 0.2;
        signals.push("settable-value".to_string());
        kind = "input";
    }

    if confidence < 0.35 {
        return None;
    }

    Some(MeetingChatTarget {
        id: format!("ax-node-{}", node.index),
        kind: kind.to_string(),
        label: node
            .title
            .clone()
            .or_else(|| node.placeholder.clone())
            .or_else(|| node.description.clone())
            .or_else(|| node.value.clone()),
        bounds: node.bounds.clone(),
        enabled: node.enabled,
        settable: node.settable_value,
        confidence,
        signals,
    })
}

#[cfg(target_os = "macos")]
impl From<cg::Rect> for AxRect {
    fn from(rect: cg::Rect) -> Self {
        Self {
            x: rect.origin.x,
            y: rect.origin.y,
            width: rect.size.width,
            height: rect.size.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(index: usize, role: &str, title: &str, bounds: Option<AxRect>) -> AxNode {
        AxNode {
            index,
            role: Some(role.to_string()),
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
        }
    }

    #[test]
    fn test_participant_name_from_zoom_video_render_label() {
        assert_eq!(
            participant_name_from_label(Some("Video render Ada Lovelace, Computer audio unmuted")),
            Some("Ada Lovelace".to_string())
        );
    }

    #[test]
    fn test_zoom_video_render_becomes_active_stream_candidate() {
        let nodes = vec![node(
            7,
            "AXGroup",
            "Video render Ada Lovelace, Computer audio unmuted",
            Some(AxRect {
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 180.0,
            }),
        )];

        let streams =
            find_participant_streams(&MeetingPlatform::Zoom, &MeetingSurface::Native, &nodes);

        assert_eq!(streams.len(), 1);
        assert_eq!(
            streams[0].participant_name,
            Some("Ada Lovelace".to_string())
        );
        assert!(streams[0].is_active_speaker);
        assert!(streams[0].confidence > 0.6);
    }

    #[test]
    fn test_chat_input_candidate_requires_chat_signal() {
        let mut chat = node(3, "AXTextArea", "Send a message", None);
        chat.settable_value = true;
        chat.text = node_text(
            &chat.role,
            &chat.title,
            &chat.value,
            &chat.description,
            &chat.placeholder,
        );

        let target = candidate_chat_target(&chat).unwrap();

        assert_eq!(target.kind, "input");
        assert!(target.settable);
        assert!(target.confidence > 0.7);
    }

    #[test]
    fn test_browser_title_classifies_meet_web() {
        assert_eq!(
            classify_platform(
                "com.google.Chrome",
                Some("Team sync - Google Meet - Google Chrome"),
                &[],
                MeetingPlatform::Unknown,
            ),
            MeetingPlatform::GoogleMeet
        );
        assert_eq!(
            classify_surface("com.google.Chrome", &MeetingPlatform::GoogleMeet),
            MeetingSurface::Web
        );
    }
}
