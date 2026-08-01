#[cfg(target_os = "macos")]
use std::collections::HashSet;

#[cfg(target_os = "macos")]
use cidre::{arc, ax, cf, cg, ns};

#[cfg(target_os = "macos")]
mod analysis;

#[cfg(target_os = "macos")]
use analysis::{
    candidate_chat_target, chat_scope_label, extract_chat_messages, find_participant_streams,
    is_explicit_chat_input, is_slack_huddle_scope_node, is_zoom_chat_scope_node,
    is_zoom_meeting_evidence, is_zoom_meeting_scope_node, meeting_chat_surface_is_visible,
};
#[cfg(all(test, target_os = "macos"))]
use analysis::{
    candidate_stream, extract_links, looks_like_time, meeting_chat_direction, parse_chat_message,
    participant_name_from_evidence, slack_huddle_is_active,
};

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum MeetingAppBundleKind {
    Native,
    Browser,
}

#[cfg(target_os = "macos")]
struct MeetingAppBundle {
    id: &'static str,
    kind: MeetingAppBundleKind,
}

#[cfg(target_os = "macos")]
impl MeetingAppBundle {
    const fn native(id: &'static str) -> Self {
        Self {
            id,
            kind: MeetingAppBundleKind::Native,
        }
    }

    const fn browser(id: &'static str) -> Self {
        Self {
            id,
            kind: MeetingAppBundleKind::Browser,
        }
    }
}

#[cfg(target_os = "macos")]
const MEETING_APP_BUNDLES: &[MeetingAppBundle] = &[
    MeetingAppBundle::native("us.zoom.xos"),
    MeetingAppBundle::native("com.microsoft.teams2"),
    MeetingAppBundle::native("com.microsoft.teams"),
    MeetingAppBundle::native("com.tinyspeck.slackmacgap"),
    MeetingAppBundle::native("com.slack.Slack"),
    MeetingAppBundle::native("com.hnc.Discord"),
    MeetingAppBundle::native("com.discordapp.Discord"),
    MeetingAppBundle::native("Cisco-Systems.Spark"),
    MeetingAppBundle::native("com.cisco.webex"),
    MeetingAppBundle::native("com.cisco.webexmeetingsapp"),
    MeetingAppBundle::browser("com.google.Chrome"),
    MeetingAppBundle::browser("com.google.Chrome.canary"),
    MeetingAppBundle::browser("com.microsoft.edgemac"),
    MeetingAppBundle::browser("com.microsoft.edgemac.Beta"),
    MeetingAppBundle::browser("com.microsoft.edgemac.Canary"),
    MeetingAppBundle::browser("com.microsoft.edgemac.Dev"),
    MeetingAppBundle::browser("org.mozilla.firefox"),
    MeetingAppBundle::browser("org.mozilla.firefoxdeveloperedition"),
    MeetingAppBundle::browser("org.mozilla.nightly"),
    MeetingAppBundle::browser("com.apple.Safari"),
    MeetingAppBundle::browser("com.apple.SafariTechnologyPreview"),
    MeetingAppBundle::browser("com.brave.Browser"),
    MeetingAppBundle::browser("com.brave.Browser.beta"),
    MeetingAppBundle::browser("com.brave.Browser.nightly"),
    MeetingAppBundle::browser("org.chromium.Chromium"),
    MeetingAppBundle::browser("com.vivaldi.Vivaldi"),
    MeetingAppBundle::browser("com.operasoftware.Opera"),
    MeetingAppBundle::browser("com.operasoftware.OperaDeveloper"),
    MeetingAppBundle::browser("com.operasoftware.OperaGX"),
    MeetingAppBundle::browser("com.operasoftware.OperaNext"),
    MeetingAppBundle::browser("company.thebrowser.Browser"),
    MeetingAppBundle::browser("ai.perplexity.comet"),
    MeetingAppBundle::browser("at.studio.AsideBrowser"),
    MeetingAppBundle::browser("company.thebrowser.dia"),
    MeetingAppBundle::browser("com.sigmaos.sigmaos.macos"),
    MeetingAppBundle::browser("net.imput.helium"),
    MeetingAppBundle::browser("com.nousresearch.hermes"),
];

#[cfg(target_os = "macos")]
const MAX_TREE_DEPTH: usize = 18;
#[cfg(target_os = "macos")]
const MAX_NODES: usize = 1800;
#[cfg(target_os = "macos")]
const MIN_VIDEO_AREA: f64 = 18_000.0;
const MAX_MEETING_CHAT_MESSAGE_CHARS: usize = 2_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MeetingPlatform {
    Zoom,
    GoogleMeet,
    MicrosoftTeams,
    Slack,
    Discord,
    Webex,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MeetingSurface {
    Native,
    Web,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MeetingChatDirection {
    Incoming,
    Outgoing,
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

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct MeetingChatTarget {
    kind: String,
    #[cfg(test)]
    settable: bool,
    confidence: f32,
    #[cfg(test)]
    signals: Vec<String>,
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
    pub active_speakers: Vec<String>,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingCapturedChatMessage {
    pub id: String,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub sender: Option<String>,
    pub timestamp: Option<String>,
    pub direction: Option<MeetingChatDirection>,
    pub text: String,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingChatCaptureResult {
    pub app: Option<MeetingApp>,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub context_id: Option<String>,
    pub messages: Vec<MeetingCapturedChatMessage>,
    pub warnings: Vec<String>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct AxNode {
    index: usize,
    tree_path: Vec<usize>,
    element_hash: Option<usize>,
    role: Option<String>,
    identifier: Option<String>,
    title: Option<String>,
    value: Option<String>,
    description: Option<String>,
    placeholder: Option<String>,
    enabled: Option<bool>,
    settable_value: bool,
    bounds: Option<AxRect>,
    text: String,
    within_zoom_meeting_scope: bool,
    within_zoom_chat_scope: bool,
    within_slack_huddle_scope: bool,
}

#[cfg(target_os = "macos")]
struct AxChatElement {
    node: AxNode,
    ancestors: Vec<AxAncestor>,
    element: arc::R<ax::UiElement>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct AxAncestor {
    path: Vec<usize>,
    labels: Vec<String>,
}

#[cfg(target_os = "macos")]
struct SlackHuddleRoot {
    channel: String,
    label: String,
    nodes: Vec<AxNode>,
    element: arc::R<ax::UiElement>,
}

#[cfg(target_os = "macos")]
struct BrowserMeetingRoot {
    platform: MeetingPlatform,
    window_title: Option<String>,
    web_area_url: Option<String>,
    nodes: Vec<AxNode>,
}

#[cfg(target_os = "macos")]
struct NativeMeetingRoot {
    window_title: Option<String>,
    nodes: Vec<AxNode>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, PartialEq, Eq)]
enum UniqueMatch {
    Missing,
    One(usize),
    Ambiguous,
}

#[cfg(target_os = "macos")]
fn unique_scope_for_count(count: usize) -> UniqueMatch {
    match count {
        0 => UniqueMatch::Missing,
        1 => UniqueMatch::One(0),
        _ => UniqueMatch::Ambiguous,
    }
}

#[cfg(target_os = "macos")]
fn unique_scope_for_search(count: usize, complete: bool) -> UniqueMatch {
    if complete {
        unique_scope_for_count(count)
    } else {
        UniqueMatch::Ambiguous
    }
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

fn validate_meeting_chat_message(message: &str) -> Result<(), &'static str> {
    if message.trim().is_empty() {
        return Err("meeting chat message must not be empty");
    }

    if message.chars().count() > MAX_MEETING_CHAT_MESSAGE_CHARS {
        return Err("meeting chat message exceeds the 2000 character safety limit");
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn send_meeting_chat_message(
    message: String,
    mic_active_bundle_ids: Vec<String>,
) -> MeetingChatSendResult {
    if let Err(warning) = validate_meeting_chat_message(&message) {
        return MeetingChatSendResult {
            sent: false,
            app: None,
            platform: MeetingPlatform::Unknown,
            surface: MeetingSurface::Unknown,
            input_label: None,
            send_action: None,
            warnings: vec![warning.to_string()],
        };
    }

    let scoped_bundle_id = match unique_recognized_meeting_bundle(&mic_active_bundle_ids) {
        Ok(bundle_id) => bundle_id,
        Err(warning) => {
            return MeetingChatSendResult {
                sent: false,
                app: None,
                platform: MeetingPlatform::Unknown,
                surface: MeetingSurface::Unknown,
                input_label: None,
                send_action: None,
                warnings: vec![warning],
            };
        }
    };
    let scoped_platform = classify_bundle(scoped_bundle_id);
    let scoped_surface = classify_surface(scoped_bundle_id, &scoped_platform);
    if !supports_meeting_chat_mutation(scoped_bundle_id) {
        return MeetingChatSendResult {
            sent: false,
            app: None,
            platform: scoped_platform,
            surface: scoped_surface,
            input_label: None,
            send_action: None,
            warnings: vec![format!(
                "AX chat mutation is disabled for the mic-active meeting app {scoped_bundle_id}"
            )],
        };
    }

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

    let mut validated_apps = Vec::new();
    let mut warnings = Vec::new();
    for (app, pid) in running_apps_for_bundle(scoped_bundle_id) {
        let ax_app = ax::UiElement::with_app_pid(pid);
        let _ = ax_app.set_messaging_timeout_secs(0.6);
        let mut roots = collect_slack_huddle_roots(&ax_app, &mut warnings);
        if roots.len() > 1 {
            warnings.push(format!(
                "refusing to send because Slack exposed {} active Huddle windows",
                roots.len()
            ));
            return slack_chat_failure(
                &app,
                &classify_surface(&app.id, &MeetingPlatform::Slack),
                None,
                warnings,
            );
        }
        if let Some(root) = roots.pop() {
            validated_apps.push((app, root));
        }
    }

    if validated_apps.len() > 1 {
        return MeetingChatSendResult {
            sent: false,
            app: None,
            platform: MeetingPlatform::Slack,
            surface: MeetingSurface::Unknown,
            input_label: None,
            send_action: None,
            warnings: vec![
                "refusing to send because multiple running Slack apps expose active Huddles"
                    .to_string(),
            ],
        };
    }

    if let Some((app, root)) = validated_apps.pop() {
        let surface = classify_surface(&app.id, &MeetingPlatform::Slack);
        return send_slack_huddle_chat_message(&app, &surface, root, &message, warnings);
    }

    MeetingChatSendResult {
        sent: false,
        app: None,
        platform: MeetingPlatform::Unknown,
        surface: MeetingSurface::Unknown,
        input_label: None,
        send_action: None,
        warnings: vec![
            "no uniquely validated Slack Huddle is active; AX chat mutation for other meeting platforms is disabled until their window and composer can be paired safely"
                .to_string(),
        ],
    }
}

#[cfg(not(target_os = "macos"))]
pub fn send_meeting_chat_message(
    _message: String,
    _mic_active_bundle_ids: Vec<String>,
) -> MeetingChatSendResult {
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
fn unique_recognized_meeting_bundle(mic_active_bundle_ids: &[String]) -> Result<&str, String> {
    let recognized = mic_active_bundle_ids
        .iter()
        .map(String::as_str)
        .filter(|bundle_id| is_meeting_app_bundle(bundle_id))
        .collect::<HashSet<_>>();

    if recognized.len() != 1 {
        return Err(format!(
            "refusing to send because the mic-active apps contain {} recognized meeting app bundles; expected exactly one",
            recognized.len()
        ));
    }

    Ok(recognized.into_iter().next().unwrap())
}

#[cfg(target_os = "macos")]
fn running_apps_for_bundle(bundle_id: &str) -> Vec<(MeetingApp, i32)> {
    let mut apps = Vec::new();
    let bundle = ns::String::with_str(bundle_id);
    let running = ns::RunningApp::with_bundle_id(&bundle);

    for app in running.iter() {
        let pid = app.pid();
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

    apps
}

#[cfg(target_os = "macos")]
fn running_meeting_apps() -> Vec<(MeetingApp, i32)> {
    let mut seen = HashSet::new();

    MEETING_APP_BUNDLES
        .iter()
        .flat_map(|bundle| running_apps_for_bundle(bundle.id))
        .filter(|(_, pid)| seen.insert(*pid))
        .collect()
}

#[cfg(target_os = "macos")]
fn select_active_bundle_ids<'a>(
    supported_bundle_ids: impl IntoIterator<Item = &'a str>,
    active_bundle_ids: &[String],
) -> Vec<&'a str> {
    let active_bundle_ids = active_bundle_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    supported_bundle_ids
        .into_iter()
        .filter(|bundle_id| active_bundle_ids.contains(bundle_id))
        .collect()
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn normalized_context_part(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn path_is_ancestor(ancestor: &[usize], descendant: &[usize]) -> bool {
    ancestor.len() < descendant.len() && descendant.starts_with(ancestor)
}

#[cfg(target_os = "macos")]
fn common_tree_path(left: &[usize], right: &[usize]) -> Vec<usize> {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .map(|(part, _)| *part)
        .collect()
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn is_platform_chat_message_list(platform: &MeetingPlatform, node: &AxNode) -> bool {
    is_chat_message_list(node) && is_platform_chat_scope_container(platform, node)
}

#[cfg(target_os = "macos")]
fn node_has_positive_bounds(node: &AxNode) -> bool {
    node.bounds
        .as_ref()
        .is_some_and(|bounds| bounds.width > 0.0 && bounds.height > 0.0)
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn validated_chat_scope(
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn browser_capture_context_id(root: &BrowserMeetingRoot) -> Option<String> {
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

#[cfg(target_os = "macos")]
fn native_capture_context_id(
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

#[cfg(target_os = "macos")]
fn slack_capture_context_id(
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn zoom_capture_context_id(root: &NativeMeetingRoot) -> Option<String> {
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

#[cfg(target_os = "macos")]
fn zoom_chat_surface_is_visible(nodes: &[AxNode]) -> bool {
    meeting_chat_surface_is_visible(&MeetingPlatform::Zoom, nodes)
}

#[cfg(target_os = "macos")]
fn slack_huddle_thread_capture_nodes(root: &SlackHuddleRoot) -> Option<(Vec<AxNode>, String)> {
    let chat_elements = collect_sorted_chat_elements(&root.element);
    let composer_index = match unique_matching_chat_element_index(&chat_elements, |element| {
        is_slack_huddle_composer_in_thread(&element.node, &element.ancestors, &root.channel)
    }) {
        UniqueMatch::One(index) => index,
        UniqueMatch::Missing | UniqueMatch::Ambiguous => return None,
    };
    let composer_hash = chat_elements[composer_index]
        .node
        .element_hash
        .unwrap_or_else(|| chat_elements[composer_index].element.hash());
    let context_id = slack_capture_context_id(
        &root.channel,
        &root.label,
        root.element.hash(),
        composer_hash,
    );

    let mut scoped_nodes = Vec::new();
    let mut ancestors = Vec::new();
    let mut path = Vec::new();
    let mut visited = 0;
    collect_nodes_with_ancestors(
        &root.element,
        0,
        &mut visited,
        &mut path,
        &mut ancestors,
        &mut scoped_nodes,
    );

    let mut nodes = root
        .nodes
        .iter()
        .filter(|node| is_enabled_slack_leave_control(node))
        .cloned()
        .collect::<Vec<_>>();
    nodes.extend(
        scoped_nodes
            .into_iter()
            .filter_map(|(mut node, ancestors)| {
                slack_thread_container_path(&ancestors, &root.channel)?;
                node.within_slack_huddle_scope = true;
                Some(node)
            }),
    );
    Some((nodes, context_id))
}

#[cfg(target_os = "macos")]
fn collect_nodes_with_ancestors(
    element: &ax::UiElement,
    depth: usize,
    visited: &mut usize,
    path: &mut Vec<usize>,
    ancestors: &mut Vec<AxAncestor>,
    nodes: &mut Vec<(AxNode, Vec<AxAncestor>)>,
) {
    if depth > MAX_TREE_DEPTH || *visited >= MAX_NODES {
        return;
    }

    let index = *visited;
    *visited += 1;
    let mut node = snapshot_node(element, index);
    node.tree_path.clone_from(path);
    nodes.push((node.clone(), ancestors.clone()));
    ancestors.push(AxAncestor {
        path: path.clone(),
        labels: node_labels(&node).map(str::to_string).collect(),
    });

    if let Ok(children) = element.children() {
        for (child_index, child) in children.iter().enumerate() {
            path.push(child_index);
            collect_nodes_with_ancestors(child, depth + 1, visited, path, ancestors, nodes);
            path.pop();
        }
    }
    ancestors.pop();
}

#[cfg(target_os = "macos")]
pub fn capture_meeting_chat_messages(bundle_ids: Vec<String>) -> MeetingChatCaptureResult {
    let scoped_bundle_ids = select_active_bundle_ids(
        MEETING_APP_BUNDLES.iter().map(|bundle| bundle.id),
        &bundle_ids,
    );
    if scoped_bundle_ids.len() != 1 {
        return MeetingChatCaptureResult {
            app: None,
            platform: MeetingPlatform::Unknown,
            surface: MeetingSurface::Unknown,
            context_id: None,
            messages: Vec::new(),
            warnings: vec![format!(
                "meeting chat capture requires exactly one active supported meeting app; received {}",
                scoped_bundle_ids.len()
            )],
        };
    }

    if !macos_accessibility_client::accessibility::application_is_trusted() {
        return MeetingChatCaptureResult {
            app: None,
            platform: MeetingPlatform::Unknown,
            surface: MeetingSurface::Unknown,
            context_id: None,
            messages: Vec::new(),
            warnings: vec!["macOS accessibility permission is not trusted".to_string()],
        };
    }

    let bundle_id = scoped_bundle_ids[0];
    let bundle_platform = classify_bundle(bundle_id);
    let bundle_surface = classify_surface(bundle_id, &bundle_platform);
    let mut detected_platform = bundle_platform.clone();
    let mut warnings = Vec::new();
    let mut candidates = Vec::new();

    let running_apps = running_apps_for_bundle(bundle_id);
    if is_browser_bundle(bundle_id) {
        let mut browser_roots = Vec::new();
        let mut browser_scope_poisoned = false;
        for (app, pid) in running_apps {
            let ax_app = ax::UiElement::with_app_pid(pid);
            let _ = ax_app.set_messaging_timeout_secs(0.6);
            let (roots, has_unscoped_meeting_window) =
                collect_browser_meeting_roots(&ax_app, &mut warnings);
            browser_scope_poisoned |= has_unscoped_meeting_window;
            browser_roots.extend(roots.into_iter().filter_map(|root| {
                let context_id = browser_capture_context_id(&root)?;
                Some((app.clone(), root, context_id))
            }));
        }

        if browser_scope_poisoned || browser_roots.len() != 1 {
            warnings.push(format!(
                "browser chat capture requires exactly one completely scoped meeting root; found {}",
                browser_roots.len()
            ));
        } else {
            let (app, root, context_id) = browser_roots.pop().unwrap();
            detected_platform = root.platform.clone();
            candidates.push((
                app,
                root.platform,
                MeetingSurface::Web,
                context_id,
                root.nodes,
            ));
        }
    } else {
        for (app, pid) in running_apps {
            let ax_app = ax::UiElement::with_app_pid(pid);
            let _ = ax_app.set_messaging_timeout_secs(0.6);

            match &bundle_platform {
                MeetingPlatform::Zoom => {
                    for root in
                        collect_native_meeting_roots(&ax_app, &MeetingPlatform::Zoom, &mut warnings)
                    {
                        if zoom_chat_surface_is_visible(&root.nodes)
                            && let Some(context_id) = zoom_capture_context_id(&root)
                        {
                            candidates.push((
                                app.clone(),
                                MeetingPlatform::Zoom,
                                MeetingSurface::Native,
                                context_id,
                                root.nodes,
                            ));
                        }
                    }
                }
                MeetingPlatform::Slack => {
                    for root in collect_slack_huddle_roots(&ax_app, &mut warnings) {
                        if let Some((nodes, context_id)) = slack_huddle_thread_capture_nodes(&root)
                        {
                            candidates.push((
                                app.clone(),
                                MeetingPlatform::Slack,
                                MeetingSurface::Native,
                                context_id,
                                nodes,
                            ));
                        }
                    }
                }
                MeetingPlatform::MicrosoftTeams | MeetingPlatform::Webex => {
                    for root in
                        collect_native_meeting_roots(&ax_app, &bundle_platform, &mut warnings)
                    {
                        if let Some(context_id) = native_capture_context_id(&bundle_platform, &root)
                        {
                            candidates.push((
                                app.clone(),
                                bundle_platform.clone(),
                                MeetingSurface::Native,
                                context_id,
                                root.nodes,
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if candidates.len() != 1 {
        warnings.push(format!(
            "meeting chat capture requires exactly one validated visible chat surface; found {}",
            candidates.len()
        ));
        return MeetingChatCaptureResult {
            app: None,
            platform: detected_platform,
            surface: bundle_surface,
            context_id: None,
            messages: Vec::new(),
            warnings,
        };
    }

    let (app, platform, surface, context_id, nodes) = candidates.pop().unwrap();
    let messages = extract_chat_messages(&platform, &surface, &nodes);
    MeetingChatCaptureResult {
        app: Some(app),
        platform,
        surface,
        context_id: Some(context_id),
        messages,
        warnings,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn capture_meeting_chat_messages(_bundle_ids: Vec<String>) -> MeetingChatCaptureResult {
    MeetingChatCaptureResult {
        app: None,
        platform: MeetingPlatform::Unknown,
        surface: MeetingSurface::Unknown,
        context_id: None,
        messages: Vec::new(),
        warnings: vec!["meeting chat AX capture is only available on macOS".to_string()],
    }
}

#[cfg(target_os = "macos")]
fn send_slack_huddle_chat_message(
    app: &MeetingApp,
    surface: &MeetingSurface,
    mut root: SlackHuddleRoot,
    message: &str,
    mut warnings: Vec<String>,
) -> MeetingChatSendResult {
    let mut refreshed_nodes = Vec::new();
    if !collect_nodes(&root.element, 0, &mut refreshed_nodes, &mut warnings) {
        warnings.push("refusing to send from an incomplete Slack Huddle AX snapshot".to_string());
        return slack_chat_failure(app, surface, None, warnings);
    }
    let Some((label, channel)) = slack_huddle_context(&refreshed_nodes) else {
        warnings.push("the validated Slack Huddle changed before send".to_string());
        return slack_chat_failure(app, surface, None, warnings);
    };
    if channel != root.channel {
        warnings.push(format!(
            "the validated Slack Huddle changed from {} to {channel} before send",
            root.channel
        ));
        return slack_chat_failure(app, surface, None, warnings);
    }
    root.label = label;
    root.nodes = refreshed_nodes;
    let mut chat_elements = collect_sorted_chat_elements(&root.element);
    let mut input_match = unique_matching_chat_element_index(&chat_elements, |element| {
        is_slack_huddle_composer_in_thread(&element.node, &element.ancestors, &root.channel)
    });

    if input_match == UniqueMatch::Missing {
        match unique_matching_chat_element_index(&chat_elements, |element| {
            is_slack_thread_control(&element.node)
        }) {
            UniqueMatch::One(control_index) => {
                let control = &chat_elements[control_index];
                let label = inspection_label(&control.node)
                    .unwrap_or_else(|| "Slack Huddle thread control".to_string());

                match control.element.perform_action(ax::action::press()) {
                    Ok(_) => {
                        warnings.push(format!("opened Slack Huddle thread via AX: {label}"));
                        (chat_elements, input_match) =
                            collect_until_unique_match(&root.element, |element| {
                                is_slack_huddle_composer_in_thread(
                                    &element.node,
                                    &element.ancestors,
                                    &root.channel,
                                )
                            });
                    }
                    Err(error) => {
                        warnings.push(format!(
                            "failed to open Slack Huddle thread via AX: {error:?}"
                        ));
                        return slack_chat_failure(app, surface, None, warnings);
                    }
                }
            }
            UniqueMatch::Missing => {
                warnings.push(
                    "validated Slack Huddle did not expose its composer or thread control"
                        .to_string(),
                );
                return slack_chat_failure(app, surface, None, warnings);
            }
            UniqueMatch::Ambiguous => {
                warnings.push(
                    "validated Slack Huddle exposed multiple thread controls; refusing to open one"
                        .to_string(),
                );
                return slack_chat_failure(app, surface, None, warnings);
            }
        }
    }

    let input_index = match input_match {
        UniqueMatch::One(index) => index,
        UniqueMatch::Missing => {
            warnings.push(format!(
                "Slack Huddle thread did not expose the expected composer for {}",
                root.channel
            ));
            return slack_chat_failure(app, surface, None, warnings);
        }
        UniqueMatch::Ambiguous => {
            warnings.push(format!(
                "Slack Huddle exposed multiple composers for {}; refusing to choose one",
                root.channel
            ));
            return slack_chat_failure(app, surface, None, warnings);
        }
    };

    let input = &chat_elements[input_index];
    let Some(thread_container_path) =
        slack_thread_container_path(&input.ancestors, &root.channel).map(<[usize]>::to_vec)
    else {
        warnings.push("Slack Huddle composer lost its thread container before send".to_string());
        return slack_chat_failure(app, surface, None, warnings);
    };
    let label = inspection_label(&input.node);
    let mut input_element = input.element.retained();
    let _ = input_element.perform_action(ax::action::press());
    let original_value = match chat_input_value(&input_element) {
        Ok(value) if value.trim().is_empty() => value,
        Ok(_) => {
            warnings.push("refusing to overwrite an existing Slack Huddle draft".to_string());
            return slack_chat_failure(app, surface, label, warnings);
        }
        Err(error) => {
            warnings.push(format!(
                "could not verify that the Slack Huddle composer was empty: {error}"
            ));
            return slack_chat_failure(app, surface, label, warnings);
        }
    };

    let message_value = cf::String::from_str(message);
    if let Err(error) = input_element.set_attr(ax::attr::value(), message_value.as_type_ref()) {
        restore_chat_input_if_owned(&mut input_element, message, &original_value, &mut warnings);
        warnings.push(format!(
            "failed to set Slack Huddle composer value: {error:?}"
        ));
        return slack_chat_failure(app, surface, label, warnings);
    }

    let (refreshed_elements, send_button_match) =
        collect_until_unique_match(&root.element, |element| {
            is_slack_send_now_in_thread(
                &element.node,
                &element.ancestors,
                &root.channel,
                &thread_container_path,
            )
        });
    let button_index = match send_button_match {
        UniqueMatch::One(index) => index,
        UniqueMatch::Missing => {
            restore_chat_input_if_owned(
                &mut input_element,
                message,
                &original_value,
                &mut warnings,
            );
            warnings.push(
                "Slack Huddle composer did not expose an enabled Send now button".to_string(),
            );
            return slack_chat_failure(app, surface, label, warnings);
        }
        UniqueMatch::Ambiguous => {
            restore_chat_input_if_owned(
                &mut input_element,
                message,
                &original_value,
                &mut warnings,
            );
            warnings.push(
                "Slack Huddle exposed multiple enabled Send now buttons; refusing to choose one"
                    .to_string(),
            );
            return slack_chat_failure(app, surface, label, warnings);
        }
    };

    match chat_input_value(&input_element) {
        Ok(current) if chat_input_is_owned(&current, message) => {}
        Ok(_) => {
            warnings.push(
                "Slack Huddle composer changed while preparing the disclosure message; nothing was sent or cleared"
                    .to_string(),
            );
            return slack_chat_failure(app, surface, label, warnings);
        }
        Err(error) => {
            warnings.push(format!(
                "could not revalidate the Slack Huddle composer before send: {error}"
            ));
            return slack_chat_failure(app, surface, label, warnings);
        }
    }

    let button = &refreshed_elements[button_index];
    match button.element.perform_action(ax::action::press()) {
        Ok(_) => MeetingChatSendResult {
            sent: true,
            app: Some(app.clone()),
            platform: MeetingPlatform::Slack,
            surface: surface.clone(),
            input_label: label,
            send_action: Some("sendButton".to_string()),
            warnings,
        },
        Err(error) => {
            restore_chat_input_if_owned(
                &mut input_element,
                message,
                &original_value,
                &mut warnings,
            );
            warnings.push(format!("failed to press Slack Huddle Send now: {error:?}"));
            slack_chat_failure(app, surface, label, warnings)
        }
    }
}

#[cfg(target_os = "macos")]
fn slack_chat_failure(
    app: &MeetingApp,
    surface: &MeetingSurface,
    input_label: Option<String>,
    warnings: Vec<String>,
) -> MeetingChatSendResult {
    MeetingChatSendResult {
        sent: false,
        app: Some(app.clone()),
        platform: MeetingPlatform::Slack,
        surface: surface.clone(),
        input_label,
        send_action: None,
        warnings,
    }
}

#[cfg(target_os = "macos")]
fn chat_input_value(input: &ax::UiElement) -> Result<String, String> {
    let value = input
        .attr_value(ax::attr::value())
        .map_err(|error| format!("{error:?}"))?;
    value
        .try_as_string()
        .map(|value| value.to_string())
        .ok_or_else(|| "AXValue was not a string".to_string())
}

#[cfg(target_os = "macos")]
fn restore_chat_input_if_owned(
    input: &mut arc::R<ax::UiElement>,
    injected_message: &str,
    original_value: &str,
    warnings: &mut Vec<String>,
) {
    match chat_input_value(input) {
        Ok(current) if chat_input_is_owned(&current, injected_message) => {
            let original = cf::String::from_str(original_value);
            if let Err(error) = input.set_attr(ax::attr::value(), original.as_type_ref()) {
                warnings.push(format!(
                    "failed to restore the unsent Slack Huddle composer: {error:?}"
                ));
            }
        }
        Ok(_) => warnings.push(
            "Slack Huddle composer changed concurrently; its current value was left untouched"
                .to_string(),
        ),
        Err(error) => warnings.push(format!(
            "could not verify ownership of the Slack Huddle composer during cleanup: {error}"
        )),
    }
}

#[cfg(target_os = "macos")]
fn collect_slack_huddle_roots(
    ax_app: &ax::UiElement,
    warnings: &mut Vec<String>,
) -> Vec<SlackHuddleRoot> {
    let mut windows = Vec::new();
    let mut visited = 0;
    if !collect_window_elements(ax_app, 0, &mut visited, &mut windows) {
        warnings.push("AX window discovery was incomplete; no Slack Huddle was scoped".to_string());
        return Vec::new();
    }

    windows
        .into_iter()
        .filter_map(|element| {
            let mut nodes = Vec::new();
            if !collect_nodes(&element, 0, &mut nodes, warnings) {
                return None;
            }
            let (label, channel) = slack_huddle_context(&nodes)?;
            Some(SlackHuddleRoot {
                channel,
                label,
                nodes,
                element,
            })
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn collect_native_meeting_roots(
    ax_app: &ax::UiElement,
    platform: &MeetingPlatform,
    warnings: &mut Vec<String>,
) -> Vec<NativeMeetingRoot> {
    let mut windows = Vec::new();
    let mut visited = 0;
    if !collect_window_elements(ax_app, 0, &mut visited, &mut windows) {
        warnings
            .push("AX window discovery was incomplete; no native meeting was scoped".to_string());
        return Vec::new();
    }

    windows
        .into_iter()
        .filter_map(|element| {
            let window_title = string_attr(&element, ax::attr::title());
            let mut nodes = Vec::new();
            if !collect_nodes(&element, 0, &mut nodes, warnings) {
                return None;
            }
            native_meeting_window_is_validated(platform, &nodes).then_some(NativeMeetingRoot {
                window_title,
                nodes,
            })
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn native_meeting_window_is_validated(platform: &MeetingPlatform, nodes: &[AxNode]) -> bool {
    match platform {
        MeetingPlatform::Zoom => nodes.iter().any(is_zoom_meeting_evidence),
        MeetingPlatform::Discord => nodes.iter().any(|node| {
            node_labels(node).any(|label| label.trim().eq_ignore_ascii_case("voice connected"))
        }),
        MeetingPlatform::MicrosoftTeams | MeetingPlatform::Webex => nodes
            .iter()
            .any(|node| is_platform_active_call_control(platform, node)),
        MeetingPlatform::GoogleMeet | MeetingPlatform::Unknown => false,
        MeetingPlatform::Slack => slack_huddle_context(nodes).is_some(),
    }
}

#[cfg(target_os = "macos")]
fn collect_browser_meeting_roots(
    ax_app: &ax::UiElement,
    warnings: &mut Vec<String>,
) -> (Vec<BrowserMeetingRoot>, bool) {
    let focused_web_area = focused_web_area_element(ax_app);
    let mut windows = Vec::new();
    let mut visited = 0;
    let mut has_unscoped_meeting_window = false;
    if !collect_window_elements(ax_app, 0, &mut visited, &mut windows) {
        warnings.push(
            "browser AX window discovery was incomplete; browser capture was excluded".to_string(),
        );
        return (Vec::new(), true);
    }

    let roots = windows
        .into_iter()
        .filter_map(|window| {
            let window_title = string_attr(&window, ax::attr::title());
            let (web_area, web_area_search_complete) =
                active_web_area_element(&window, focused_web_area.as_deref());
            if !web_area_search_complete {
                if browser_window_has_provider_signal(None, window_title.as_deref()) {
                    has_unscoped_meeting_window = true;
                    warnings.push(
                        "a meeting-like browser window had an incomplete AXWebArea search; browser capture was excluded"
                            .to_string(),
                    );
                }
                return None;
            }
            let Some(web_area) = web_area else {
                if window_title
                    .as_deref()
                    .is_some_and(|title| !browser_title_platform_signals(title).is_empty())
                {
                    has_unscoped_meeting_window = true;
                    warnings.push(
                        "a meeting-like browser window did not expose one active AXWebArea; it was excluded"
                            .to_string(),
                    );
                }
                return None;
            };

            let web_area_node = snapshot_node(&web_area, 0);
            let web_area_url = url_attr(&web_area).or_else(|| {
                web_area_node.value.as_ref().and_then(|value| {
                    value
                        .starts_with("http")
                        .then_some(value.clone())
                })
            });
            let mut nodes = Vec::new();
            let mut root_warnings = Vec::new();
            if !collect_nodes(&web_area, 0, &mut nodes, &mut root_warnings) {
                if browser_window_has_provider_signal(
                    web_area_url.as_deref(),
                    window_title.as_deref(),
                ) {
                    has_unscoped_meeting_window = true;
                    warnings.extend(root_warnings);
                }
                return None;
            }
            warnings.extend(root_warnings);
            let platform = classify_browser_context(
                web_area_url.as_deref(),
                window_title.as_deref(),
                Some(&web_area_node),
                &nodes,
            );
            if platform == MeetingPlatform::Unknown {
                if browser_window_has_provider_signal(
                    web_area_url.as_deref(),
                    window_title.as_deref(),
                ) {
                    warnings.push(
                        "a browser window lacked matching meeting-origin and title/control signals; it was excluded"
                            .to_string(),
                    );
                }
                return None;
            }

            Some(BrowserMeetingRoot {
                platform,
                window_title,
                web_area_url,
                nodes,
            })
        })
        .collect();

    (roots, has_unscoped_meeting_window)
}

#[cfg(target_os = "macos")]
fn browser_window_has_provider_signal(url: Option<&str>, title: Option<&str>) -> bool {
    browser_platform_from_url(url).is_some()
        || title.is_some_and(|title| !browser_title_platform_signals(title).is_empty())
}

#[cfg(target_os = "macos")]
fn focused_web_area_element(ax_app: &ax::UiElement) -> Option<arc::R<ax::UiElement>> {
    let mut element = ax_app.focused_ui_element().ok()?;
    for _ in 0..=MAX_TREE_DEPTH {
        if element
            .role()
            .ok()
            .is_some_and(|role| role.to_string() == "AXWebArea")
        {
            return Some(element);
        }
        element = element.parent().ok()?;
    }
    None
}

#[cfg(target_os = "macos")]
fn active_web_area_element(
    window: &ax::UiElement,
    focused_web_area: Option<&ax::UiElement>,
) -> (Option<arc::R<ax::UiElement>>, bool) {
    if let Some(focused_web_area) = focused_web_area {
        let belongs_to_window = focused_web_area
            .window()
            .ok()
            .is_some_and(|focused_window| focused_window.equal(window));
        if belongs_to_window {
            return (Some(focused_web_area.retained()), true);
        }
    }

    let mut web_areas = Vec::new();
    let mut visited = 0;
    let complete = collect_web_area_elements(window, 0, &mut visited, &mut web_areas);
    match unique_scope_for_search(web_areas.len(), complete) {
        UniqueMatch::One(index) => (Some(web_areas.remove(index)), true),
        UniqueMatch::Missing | UniqueMatch::Ambiguous => (None, complete),
    }
}

#[cfg(target_os = "macos")]
fn collect_web_area_elements(
    element: &ax::UiElement,
    depth: usize,
    visited: &mut usize,
    web_areas: &mut Vec<arc::R<ax::UiElement>>,
) -> bool {
    if depth > MAX_TREE_DEPTH || *visited >= MAX_NODES {
        return false;
    }
    *visited += 1;

    let Ok(role) = element.role() else {
        return false;
    };
    let role = role.to_string();
    if role == "AXWebArea" {
        web_areas.push(element.retained());
        return true;
    }

    let Ok(children) = element.children() else {
        return !ax_role_may_have_children(&role);
    };
    let mut complete = true;
    for child in children.iter() {
        complete &= collect_web_area_elements(child, depth + 1, visited, web_areas);
    }
    complete
}

#[cfg(target_os = "macos")]
fn collect_window_elements(
    element: &ax::UiElement,
    depth: usize,
    visited: &mut usize,
    windows: &mut Vec<arc::R<ax::UiElement>>,
) -> bool {
    if depth > MAX_TREE_DEPTH || *visited >= MAX_NODES {
        return false;
    }
    *visited += 1;

    let Ok(role) = element.role() else {
        return false;
    };
    let role = role.to_string();
    if role == "AXWindow" {
        windows.push(element.retained());
        return true;
    }

    let Ok(children) = element.children() else {
        return !ax_role_may_have_children(&role);
    };
    let mut complete = true;
    for child in children.iter() {
        complete &= collect_window_elements(child, depth + 1, visited, windows);
    }
    complete
}

#[cfg(target_os = "macos")]
fn ax_role_may_have_children(role: &str) -> bool {
    matches!(
        role,
        "AXApplication"
            | "AXWindow"
            | "AXGroup"
            | "AXWebArea"
            | "AXScrollArea"
            | "AXList"
            | "AXTable"
            | "AXOutline"
            | "AXRow"
            | "AXCell"
            | "AXSheet"
            | "AXLandmark"
            | "AXSplitGroup"
            | "AXToolbar"
            | "AXTabGroup"
            | "AXMenuBar"
            | "AXMenu"
            | "AXPopover"
            | "AXBrowser"
            | "AXLayoutArea"
    )
}

#[cfg(all(target_os = "macos", test))]
fn unique_matching_index<'a>(
    nodes: impl Iterator<Item = (usize, &'a AxNode)>,
    predicate: impl Fn(&AxNode) -> bool,
) -> UniqueMatch {
    let mut found = None;
    for (index, node) in nodes {
        if !predicate(node) {
            continue;
        }
        if found.is_some() {
            return UniqueMatch::Ambiguous;
        }
        found = Some(index);
    }

    found.map_or(UniqueMatch::Missing, UniqueMatch::One)
}

#[cfg(target_os = "macos")]
fn unique_matching_chat_element_index(
    elements: &[AxChatElement],
    predicate: impl Fn(&AxChatElement) -> bool,
) -> UniqueMatch {
    let mut found = None;
    for (index, element) in elements.iter().enumerate() {
        if !predicate(element) {
            continue;
        }
        if found.is_some() {
            return UniqueMatch::Ambiguous;
        }
        found = Some(index);
    }

    found.map_or(UniqueMatch::Missing, UniqueMatch::One)
}

#[cfg(target_os = "macos")]
fn collect_until_unique_match(
    element: &ax::UiElement,
    predicate: impl Fn(&AxChatElement) -> bool,
) -> (Vec<AxChatElement>, UniqueMatch) {
    for attempt in 0..3 {
        let elements = collect_sorted_chat_elements(element);
        let target_match = unique_matching_chat_element_index(&elements, &predicate);
        if target_match != UniqueMatch::Missing || attempt == 2 {
            return (elements, target_match);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    unreachable!()
}

#[cfg(target_os = "macos")]
fn collect_sorted_chat_elements(element: &ax::UiElement) -> Vec<AxChatElement> {
    let mut elements = Vec::new();
    let mut ancestors = Vec::new();
    let mut path = Vec::new();
    let mut visited = 0;
    collect_chat_elements(
        element,
        0,
        &mut visited,
        &mut path,
        &mut ancestors,
        &mut elements,
    );
    elements.sort_by(|a, b| chat_element_score(&b.node).total_cmp(&chat_element_score(&a.node)));
    elements
}

#[cfg(target_os = "macos")]
fn collect_chat_elements(
    element: &ax::UiElement,
    depth: usize,
    visited: &mut usize,
    path: &mut Vec<usize>,
    ancestors: &mut Vec<AxAncestor>,
    elements: &mut Vec<AxChatElement>,
) {
    if depth > MAX_TREE_DEPTH || *visited >= MAX_NODES {
        return;
    }

    let index = *visited;
    *visited += 1;
    let mut node = snapshot_node(element, index);
    node.tree_path.clone_from(path);
    if candidate_chat_target(&node).is_some() {
        elements.push(AxChatElement {
            node: node.clone(),
            ancestors: ancestors.clone(),
            element: element.retained(),
        });
    }

    ancestors.push(AxAncestor {
        path: path.clone(),
        labels: node_labels(&node).map(str::to_string).collect(),
    });

    let Ok(children) = element.children() else {
        ancestors.pop();
        return;
    };

    for (child_index, child) in children.iter().enumerate() {
        path.push(child_index);
        collect_chat_elements(child, depth + 1, visited, path, ancestors, elements);
        path.pop();
    }
    ancestors.pop();
}

#[cfg(target_os = "macos")]
fn chat_element_score(node: &AxNode) -> f32 {
    candidate_chat_target(node)
        .map(|target| target.confidence)
        .unwrap_or(0.0)
}

#[cfg(target_os = "macos")]
fn inspection_label(node: &AxNode) -> Option<String> {
    node.title
        .clone()
        .or_else(|| node.placeholder.clone())
        .or_else(|| node.description.clone())
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
    let mut scoped_platform = None;

    if accessibility_trusted {
        let ax_app = ax::UiElement::with_app_pid(pid);
        let _ = ax_app.set_messaging_timeout_secs(0.6);
        if bundle_platform == MeetingPlatform::Slack {
            let mut roots = collect_slack_huddle_roots(&ax_app, &mut warnings);
            match roots.len() {
                0 => warnings.push(
                    "Slack is running without a uniquely validated active Huddle".to_string(),
                ),
                1 => {
                    let root = roots.remove(0);
                    window_title = Some(root.label);
                    nodes = root.nodes;
                }
                count => warnings.push(format!(
                    "Slack exposed {count} active Huddle windows; inspection is scoped to none"
                )),
            }
        } else if is_browser_bundle(&app.id) {
            let (mut roots, has_unscoped_meeting_window) =
                collect_browser_meeting_roots(&ax_app, &mut warnings);
            let root_match = if has_unscoped_meeting_window {
                UniqueMatch::Ambiguous
            } else {
                unique_scope_for_count(roots.len())
            };
            match root_match {
                UniqueMatch::Missing => warnings.push(
                    "browser inspection found no uniquely scoped meeting AXWindow and active AXWebArea"
                        .to_string(),
                ),
                UniqueMatch::One(index) => {
                    let root = roots.remove(index);
                    scoped_platform = Some(root.platform);
                    window_title = root.window_title;
                    nodes = root.nodes;
                }
                UniqueMatch::Ambiguous => warnings.push(
                    "browser meeting window scope was ambiguous; inspection is scoped to none"
                        .to_string(),
                ),
            }
        } else if bundle_platform != MeetingPlatform::Unknown {
            let mut roots = collect_native_meeting_roots(&ax_app, &bundle_platform, &mut warnings);
            match unique_scope_for_count(roots.len()) {
                UniqueMatch::Missing => {
                    warnings.push(
                        "native app exposed no evidence-backed meeting AXWindow; inspection is scoped to none"
                            .to_string(),
                    );
                }
                UniqueMatch::One(index) => {
                    let root = roots.remove(index);
                    scoped_platform = Some(bundle_platform.clone());
                    window_title = root.window_title;
                    nodes = root.nodes;
                }
                UniqueMatch::Ambiguous => {
                    warnings.push(
                        "native app exposed multiple meeting AXWindows; inspection is scoped to none"
                            .to_string(),
                    );
                }
            }
        } else {
            scoped_platform = Some(MeetingPlatform::Unknown);
            warnings.push("app bundle has no validated meeting inspection path".to_string());
        }
    } else {
        warnings.push("macOS accessibility permission is not trusted".to_string());
    }

    let platform = scoped_platform.unwrap_or_else(|| {
        classify_platform(&app.id, window_title.as_deref(), &nodes, bundle_platform)
    });
    let surface = classify_surface(&app.id, &platform);
    let participant_streams = find_participant_streams(&platform, &surface, &nodes);
    let mut active_speaker_names = HashSet::new();
    let active_speakers = participant_streams
        .iter()
        .filter(|stream| stream.is_active_speaker)
        .filter_map(|stream| {
            stream
                .participant_name
                .clone()
                .or_else(|| stream.label.clone())
        })
        .filter(|name| active_speaker_names.insert(name.trim().to_ascii_lowercase()))
        .collect();
    MeetingAccessibilityInspection {
        app,
        pid,
        platform,
        surface,
        accessibility_trusted,
        window_title,
        participant_streams,
        active_speakers,
        warnings,
    }
}

#[cfg(target_os = "macos")]
fn collect_nodes(
    element: &ax::UiElement,
    depth: usize,
    nodes: &mut Vec<AxNode>,
    warnings: &mut Vec<String>,
) -> bool {
    let mut tree_path = Vec::new();
    let mut truncated = false;
    collect_nodes_with_scope(
        element,
        depth,
        &mut tree_path,
        false,
        false,
        false,
        nodes,
        &mut truncated,
    );
    if truncated {
        warnings.push(format!(
            "AX tree snapshot was incomplete at depth {MAX_TREE_DEPTH} or {MAX_NODES} nodes"
        ));
    }
    !truncated
}

#[cfg(target_os = "macos")]
fn collect_nodes_with_scope(
    element: &ax::UiElement,
    depth: usize,
    tree_path: &mut Vec<usize>,
    within_zoom_meeting_scope: bool,
    within_zoom_chat_scope: bool,
    within_slack_huddle_scope: bool,
    nodes: &mut Vec<AxNode>,
    truncated: &mut bool,
) {
    if depth > MAX_TREE_DEPTH || nodes.len() >= MAX_NODES {
        *truncated = true;
        return;
    }

    let index = nodes.len();
    let mut node = snapshot_node(element, index);
    node.tree_path.clone_from(tree_path);
    let within_zoom_meeting_scope = within_zoom_meeting_scope || is_zoom_meeting_scope_node(&node);
    let within_zoom_chat_scope = within_zoom_chat_scope || is_zoom_chat_scope_node(&node);
    let within_slack_huddle_scope = within_slack_huddle_scope || is_slack_huddle_scope_node(&node);
    node.within_zoom_meeting_scope = within_zoom_meeting_scope;
    node.within_zoom_chat_scope = within_zoom_chat_scope;
    node.within_slack_huddle_scope = within_slack_huddle_scope;
    nodes.push(node);

    let children = match element.children() {
        Ok(children) => children,
        Err(_) => return,
    };

    for (child_index, child) in children.iter().enumerate() {
        if nodes.len() >= MAX_NODES {
            *truncated = true;
            return;
        }

        tree_path.push(child_index);
        collect_nodes_with_scope(
            child,
            depth + 1,
            tree_path,
            within_zoom_meeting_scope,
            within_zoom_chat_scope,
            within_slack_huddle_scope,
            nodes,
            truncated,
        );
        tree_path.pop();
    }
}

#[cfg(target_os = "macos")]
fn snapshot_node(element: &ax::UiElement, index: usize) -> AxNode {
    let element_hash = Some(element.hash());
    let role = element.role().ok().map(|role| role.to_string());
    let identifier = string_attr(element, ax::attr::id());
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
    let text = searchable_node_text(
        &role,
        &title,
        &value,
        &description,
        &placeholder,
        settable_value,
    );

    AxNode {
        index,
        tree_path: Vec::new(),
        element_hash,
        role,
        identifier,
        title,
        value,
        description,
        placeholder,
        enabled,
        settable_value,
        bounds,
        text,
        within_zoom_meeting_scope: false,
        within_zoom_chat_scope: false,
        within_slack_huddle_scope: false,
    }
}

#[cfg(target_os = "macos")]
fn string_attr(element: &ax::UiElement, attr: &ax::Attr) -> Option<String> {
    let value = element.attr_value(attr).ok()?;
    value.try_as_string().map(|s| s.to_string())
}

#[cfg(target_os = "macos")]
fn url_attr(element: &ax::UiElement) -> Option<String> {
    let value = element.attr_value(ax::attr::url()).ok()?;
    if let Some(value) = value.try_as_string() {
        return Some(value.to_string());
    }
    if value.get_type_id() != cf::Url::type_id() {
        return None;
    }

    let value_ref: &cf::Type = &value;
    // AXURL is a CFURL on some browsers and a CFString on others.
    let url = unsafe { &*(std::ptr::from_ref(value_ref).cast::<cf::Url>()) };
    Some(url.cf_string().to_string())
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
fn searchable_node_text(
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

#[cfg(target_os = "macos")]
fn is_text_input_role(role: &Option<String>) -> bool {
    matches!(
        role.as_deref(),
        Some("AXTextArea") | Some("AXTextField") | Some("AXSecureTextField")
    )
}

#[cfg(target_os = "macos")]
fn node_labels(node: &AxNode) -> impl Iterator<Item = &str> {
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

#[cfg(target_os = "macos")]
fn slack_huddle_context(nodes: &[AxNode]) -> Option<(String, String)> {
    let has_leave_control = nodes.iter().any(is_enabled_slack_leave_control);
    if !has_leave_control {
        return None;
    }

    nodes.iter().find_map(|node| {
        node_labels(node).find_map(|label| {
            slack_huddle_channel_from_label(label).map(|channel| (label.to_string(), channel))
        })
    })
}

#[cfg(target_os = "macos")]
fn slack_huddle_channel_from_label(label: &str) -> Option<String> {
    const PREFIX: &str = "huddle in ";

    let label = label.trim();
    let lower = label.to_ascii_lowercase();
    let start = lower.find(PREFIX)? + PREFIX.len();
    let mut channel = label[start..].trim();

    for suffix in [" (private channel)", " - slack", " | slack", " — slack"] {
        if channel.to_ascii_lowercase().ends_with(suffix) {
            channel = channel[..channel.len() - suffix.len()].trim_end();
            break;
        }
    }

    (!channel.is_empty()).then_some(channel.to_string())
}

#[cfg(target_os = "macos")]
fn is_enabled_slack_leave_control(node: &AxNode) -> bool {
    matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
        && node.enabled != Some(false)
        && node_labels(node).any(|label| label.trim().eq_ignore_ascii_case("leave huddle"))
}

#[cfg(target_os = "macos")]
fn is_slack_huddle_composer(node: &AxNode, channel: &str) -> bool {
    let expected = format!("message to {channel}");
    matches!(
        node.role.as_deref(),
        Some("AXTextArea") | Some("AXTextField")
    ) && node.enabled != Some(false)
        && node.settable_value
        && node_labels(node).any(|label| label.trim().eq_ignore_ascii_case(&expected))
}

#[cfg(target_os = "macos")]
fn slack_thread_container_path<'a>(
    ancestors: &'a [AxAncestor],
    channel: &str,
) -> Option<&'a [usize]> {
    ancestors.iter().rev().find_map(|ancestor| {
        ancestor
            .labels
            .iter()
            .find(|label| is_slack_thread_container_label(label, channel))
            .map(|_| ancestor.path.as_slice())
    })
}

#[cfg(target_os = "macos")]
fn is_slack_thread_container_label(label: &str, channel: &str) -> bool {
    let label = label.trim().to_ascii_lowercase();
    let expected = format!("thread in {}", channel.trim()).to_ascii_lowercase();
    label == expected || label.starts_with(&format!("{expected} ("))
}

#[cfg(target_os = "macos")]
fn is_slack_huddle_composer_in_thread(
    node: &AxNode,
    ancestors: &[AxAncestor],
    channel: &str,
) -> bool {
    is_slack_huddle_composer(node, channel)
        && slack_thread_container_path(ancestors, channel).is_some()
}

#[cfg(target_os = "macos")]
fn is_slack_send_now_in_thread(
    node: &AxNode,
    ancestors: &[AxAncestor],
    channel: &str,
    thread_path: &[usize],
) -> bool {
    is_slack_send_now_button(node)
        && slack_thread_container_path(ancestors, channel) == Some(thread_path)
}

#[cfg(target_os = "macos")]
fn is_slack_thread_control(node: &AxNode) -> bool {
    matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
        && node.enabled != Some(false)
        && node_labels(node).any(|label| label.trim().eq_ignore_ascii_case("show/hide thread"))
}

#[cfg(target_os = "macos")]
fn is_slack_send_now_button(node: &AxNode) -> bool {
    matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
        && node.enabled != Some(false)
        && node_labels(node).any(|label| label.trim().eq_ignore_ascii_case("send now"))
}

#[cfg(all(target_os = "macos", test))]
fn has_nonempty_draft(node: &AxNode) -> bool {
    node.value
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
}

#[cfg(target_os = "macos")]
fn chat_input_is_owned(current_value: &str, injected_message: &str) -> bool {
    current_value == injected_message
}

#[cfg(target_os = "macos")]
fn classify_bundle(bundle_id: &str) -> MeetingPlatform {
    match bundle_id {
        "us.zoom.xos" => MeetingPlatform::Zoom,
        "com.microsoft.teams2" | "com.microsoft.teams" => MeetingPlatform::MicrosoftTeams,
        "com.tinyspeck.slackmacgap" | "com.slack.Slack" => MeetingPlatform::Slack,
        "com.hnc.Discord" | "com.discordapp.Discord" => MeetingPlatform::Discord,
        "Cisco-Systems.Spark" | "com.cisco.webex" | "com.cisco.webexmeetingsapp" => {
            MeetingPlatform::Webex
        }
        _ => MeetingPlatform::Unknown,
    }
}

#[cfg(target_os = "macos")]
fn supports_meeting_chat_mutation(bundle_id: &str) -> bool {
    classify_bundle(bundle_id) == MeetingPlatform::Slack
}

#[cfg(target_os = "macos")]
fn classify_browser_context(
    web_area_url: Option<&str>,
    window_title: Option<&str>,
    active_web_area: Option<&AxNode>,
    nodes: &[AxNode],
) -> MeetingPlatform {
    let Some(platform) = browser_platform_from_url(web_area_url) else {
        return MeetingPlatform::Unknown;
    };

    let mut title_platforms = window_title
        .into_iter()
        .chain(active_web_area.into_iter().flat_map(node_labels))
        .flat_map(browser_title_platform_signals)
        .collect::<Vec<_>>();
    title_platforms.dedup();
    if title_platforms.iter().any(|signal| signal != &platform) {
        return MeetingPlatform::Unknown;
    }
    let has_matching_title = title_platforms.contains(&platform);
    let has_matching_control = nodes
        .iter()
        .any(|node| is_platform_meeting_control(&platform, node));

    if has_matching_title || has_matching_control {
        platform
    } else {
        MeetingPlatform::Unknown
    }
}

#[cfg(target_os = "macos")]
fn browser_platform_from_url(url: Option<&str>) -> Option<MeetingPlatform> {
    let url = url::Url::parse(url?).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();

    if host == "meet.google.com" {
        Some(MeetingPlatform::GoogleMeet)
    } else if matches!(host.as_str(), "teams.microsoft.com" | "teams.live.com") {
        Some(MeetingPlatform::MicrosoftTeams)
    } else if host == "zoom.us" || host.ends_with(".zoom.us") {
        Some(MeetingPlatform::Zoom)
    } else if host == "webex.com" || host.ends_with(".webex.com") {
        Some(MeetingPlatform::Webex)
    } else if matches!(host.as_str(), "slack.com" | "app.slack.com") {
        Some(MeetingPlatform::Slack)
    } else if matches!(
        host.as_str(),
        "discord.com" | "canary.discord.com" | "ptb.discord.com"
    ) {
        Some(MeetingPlatform::Discord)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn browser_title_platform_signals(text: &str) -> Vec<MeetingPlatform> {
    let text = text.to_ascii_lowercase();
    let mut platforms = Vec::new();

    if text.contains("google meet") {
        platforms.push(MeetingPlatform::GoogleMeet);
    }
    if text.contains("microsoft teams") || text.contains("teams meeting") {
        platforms.push(MeetingPlatform::MicrosoftTeams);
    }
    if text.contains("zoom meeting") {
        platforms.push(MeetingPlatform::Zoom);
    }
    if text.contains("huddle") && text.contains("slack") {
        platforms.push(MeetingPlatform::Slack);
    }
    if text.contains("discord") && (text.contains("voice") || text.contains("call")) {
        platforms.push(MeetingPlatform::Discord);
    }
    if text.contains("webex meeting") || text.contains("cisco webex") {
        platforms.push(MeetingPlatform::Webex);
    }

    platforms
}

#[cfg(target_os = "macos")]
fn is_platform_meeting_control(platform: &MeetingPlatform, node: &AxNode) -> bool {
    if !matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
        || node.enabled == Some(false)
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
                "leave meeting" | "end meeting" | "mute me" | "unmute me"
            ),
            MeetingPlatform::Unknown => false,
        }
    })
}

#[cfg(target_os = "macos")]
fn is_platform_active_call_control(platform: &MeetingPlatform, node: &AxNode) -> bool {
    if !matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
        || node.enabled == Some(false)
        || !node_has_positive_bounds(node)
    {
        return false;
    }

    node_labels(node).any(|label| {
        let label = label.trim().to_ascii_lowercase();
        match platform {
            MeetingPlatform::GoogleMeet => label == "leave call",
            MeetingPlatform::MicrosoftTeams => label == "hang up",
            MeetingPlatform::Zoom => matches!(label.as_str(), "leave meeting" | "end meeting"),
            MeetingPlatform::Slack => matches!(label.as_str(), "leave huddle" | "end huddle"),
            MeetingPlatform::Webex => matches!(label.as_str(), "leave meeting" | "end meeting"),
            MeetingPlatform::Discord | MeetingPlatform::Unknown => false,
        }
    })
}

#[cfg(target_os = "macos")]
fn classify_platform(
    bundle_id: &str,
    _window_title: Option<&str>,
    _nodes: &[AxNode],
    bundle_platform: MeetingPlatform,
) -> MeetingPlatform {
    if is_browser_bundle(bundle_id) {
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
fn meeting_app_bundle(bundle_id: &str) -> Option<&MeetingAppBundle> {
    MEETING_APP_BUNDLES
        .iter()
        .find(|bundle| bundle.id == bundle_id)
}

#[cfg(target_os = "macos")]
fn is_meeting_app_bundle(bundle_id: &str) -> bool {
    meeting_app_bundle(bundle_id).is_some()
}

#[cfg(target_os = "macos")]
fn is_browser_bundle(bundle_id: &str) -> bool {
    meeting_app_bundle(bundle_id).is_some_and(|bundle| bundle.kind == MeetingAppBundleKind::Browser)
}

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

#[cfg(all(test, target_os = "macos"))]
mod tests;
