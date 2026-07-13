#[cfg(target_os = "macos")]
use std::collections::{HashMap, HashSet};

#[cfg(target_os = "macos")]
use cidre::{arc, ax, cf, cg, ns};

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

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, PartialEq)]
struct AxRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingApp {
    pub id: String,
    pub name: String,
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
    #[cfg(test)]
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
    #[cfg(test)]
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
                #[cfg(test)]
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
fn snapshot_node(element: &ax::UiElement, _index: usize) -> AxNode {
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
        #[cfg(test)]
        index: _index,
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

#[cfg(target_os = "macos")]
fn is_zoom_meeting_evidence(node: &AxNode) -> bool {
    zoom_meeting_evidence_label(node).is_some()
}

#[cfg(target_os = "macos")]
fn zoom_meeting_evidence_label(node: &AxNode) -> Option<&str> {
    let role = node.role.as_deref()?;
    let labels = node_labels(node).collect::<Vec<_>>();
    let has_audio_state = labels.iter().any(|label| {
        let label = label.to_ascii_lowercase();
        label.contains("computer audio") || label.contains("no audio connected")
    });

    if matches!(role, "AXGroup" | "AXCell")
        && has_audio_state
        && let Some(label) = labels.iter().copied().find(|label| {
            let label = label.trim();
            let lower = label.to_ascii_lowercase();
            let is_video_render = lower
                .strip_prefix("video render ")
                .and_then(|rest| rest.split_once(','))
                .is_some_and(|(name, state)| {
                    !name.trim().is_empty()
                        && (state.contains("computer audio")
                            || state.contains("no audio connected"))
                });
            is_video_render || lower == "video tile"
        })
    {
        return Some(label);
    }

    if matches!(role, "AXStaticText" | "AXCell" | "AXRow" | "AXGroup") {
        return labels.into_iter().find(|label| {
            let lower = label.to_ascii_lowercase();
            lower.contains("participant id:")
                && (lower.contains("computer audio")
                    || lower.contains("no audio connected")
                    || lower.contains("(host")
                    || lower.contains("(me"))
        });
    }

    None
}

#[cfg(target_os = "macos")]
fn is_zoom_meeting_scope_node(node: &AxNode) -> bool {
    if node.role.as_deref() != Some("AXWindow") {
        return false;
    }

    let title = node.title.as_deref().unwrap_or_default().to_lowercase();
    title.contains("zoom meeting")
}

#[cfg(target_os = "macos")]
fn is_zoom_chat_scope_node(node: &AxNode) -> bool {
    if node.identifier.as_deref() == Some("ZMTextMessageCellView") {
        return true;
    }

    node.role.as_deref() == Some("AXTable") && chat_scope_label(node).contains("chat list")
}

#[cfg(target_os = "macos")]
fn slack_huddle_is_active(nodes: &[AxNode]) -> bool {
    nodes.iter().any(|node| {
        let role = node.role.as_deref().unwrap_or_default();
        let label = chat_scope_label(node);

        matches!(role, "AXButton" | "AXMenuItem")
            && (label.starts_with("leave huddle") || label.starts_with("end huddle"))
    })
}

#[cfg(target_os = "macos")]
fn is_slack_huddle_scope_node(node: &AxNode) -> bool {
    let role = node.role.as_deref().unwrap_or_default();
    let label = chat_scope_label(node);
    let is_huddle_chat_label = label == "huddle"
        || label.contains("huddle chat")
        || label.contains("huddle thread")
        || label.contains("huddle messages")
        || label.contains("huddle conversation");

    match role {
        "AXWindow" => is_huddle_chat_label,
        "AXGroup" | "AXScrollArea" | "AXList" | "AXWebArea" | "AXSheet" => is_huddle_chat_label,
        "AXButton" | "AXMenuItem" => {
            label.contains("open huddle chat") || label.contains("show huddle chat")
        }
        _ => false,
    }
}

#[cfg(target_os = "macos")]
fn chat_scope_label(node: &AxNode) -> String {
    [
        node.title.as_deref(),
        node.value.as_deref(),
        node.description.as_deref(),
        node.placeholder.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase()
}

#[cfg(target_os = "macos")]
fn meeting_chat_surface_is_visible(platform: &MeetingPlatform, nodes: &[AxNode]) -> bool {
    nodes.iter().any(|node| match platform {
        MeetingPlatform::Zoom => {
            node.within_zoom_meeting_scope
                && (node.within_zoom_chat_scope || is_explicit_chat_input(node))
        }
        MeetingPlatform::Slack => node.within_slack_huddle_scope && is_chat_input(node),
        _ => false,
    })
}

#[cfg(target_os = "macos")]
fn is_chat_input(node: &AxNode) -> bool {
    candidate_chat_target(node).is_some_and(|target| target.kind == "input")
}

#[cfg(target_os = "macos")]
fn is_explicit_chat_input(node: &AxNode) -> bool {
    if !is_chat_input(node) {
        return false;
    }

    let label = chat_scope_label(node);
    label.contains("send a message")
        || label.contains("message everyone")
        || label.contains("type a message")
        || label.contains("meeting chat")
}

#[cfg(target_os = "macos")]
fn is_generic_chat_message_row_or_leaf(node: &AxNode, scope_path: &[usize]) -> bool {
    node.tree_path != scope_path
        && matches!(
            node.role.as_deref(),
            Some("AXStaticText")
                | Some("AXText")
                | Some("AXCell")
                | Some("AXRow")
                | Some("AXGroup")
        )
}

#[cfg(target_os = "macos")]
fn extract_chat_messages(
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
    nodes: &[AxNode],
) -> Vec<MeetingCapturedChatMessage> {
    if *platform == MeetingPlatform::Slack && !slack_huddle_is_active(nodes) {
        return Vec::new();
    }

    let requires_generic_scope = *surface == MeetingSurface::Web
        || matches!(
            platform,
            MeetingPlatform::MicrosoftTeams | MeetingPlatform::Webex
        );
    let generic_scope_path = if requires_generic_scope {
        let Some((scope_path, _)) = validated_chat_scope(platform, nodes) else {
            return Vec::new();
        };
        Some(scope_path)
    } else {
        None
    };

    let mut parsed_nodes = Vec::new();

    for node in nodes {
        if *platform == MeetingPlatform::Zoom
            && *surface == MeetingSurface::Native
            && (!node.within_zoom_meeting_scope || !node.within_zoom_chat_scope)
        {
            continue;
        }
        if *platform == MeetingPlatform::Slack
            && *surface == MeetingSurface::Native
            && !node.within_slack_huddle_scope
        {
            continue;
        }
        if generic_scope_path
            .as_ref()
            .is_some_and(|scope_path| !node.tree_path.starts_with(scope_path))
        {
            continue;
        }
        if generic_scope_path
            .as_ref()
            .is_some_and(|scope_path| !is_generic_chat_message_row_or_leaf(node, scope_path))
        {
            continue;
        }

        let Some(raw_text) = chat_message_text(node) else {
            continue;
        };
        let Some(parsed) = parse_chat_message(platform, &raw_text) else {
            continue;
        };
        parsed_nodes.push((node, parsed));
    }

    if generic_scope_path.is_some() {
        let parseable_paths = parsed_nodes
            .iter()
            .map(|(node, _)| node.tree_path.clone())
            .collect::<Vec<_>>();
        parsed_nodes.retain(|(node, _)| {
            !parseable_paths
                .iter()
                .any(|path| path_is_ancestor(&node.tree_path, path))
        });
    }

    let mut signature_counts = HashMap::<String, usize>::new();
    let mut parsed_paths = Vec::<(String, Vec<usize>)>::new();
    let mut messages = Vec::new();

    for (node, parsed) in parsed_nodes {
        let signature = format!(
            "{:?}|{}|{}|{}",
            platform,
            parsed.sender.as_deref().unwrap_or_default(),
            parsed.timestamp.as_deref().unwrap_or_default(),
            parsed.text
        );
        if generic_scope_path.is_some()
            && parsed_paths.iter().any(|(existing_signature, path)| {
                existing_signature == &signature
                    && (path == &node.tree_path
                        || path_is_ancestor(path, &node.tree_path)
                        || path_is_ancestor(&node.tree_path, path))
            })
        {
            continue;
        }
        parsed_paths.push((signature.clone(), node.tree_path.clone()));
        let source_identity = if let Some(element_hash) = node.element_hash {
            format!("cfhash={element_hash:x}")
        } else {
            let occurrence = signature_counts.entry(signature.clone()).or_default();
            *occurrence += 1;
            format!("occurrence={occurrence}")
        };

        messages.push(MeetingCapturedChatMessage {
            id: format!("ax-chat-{signature}|{source_identity}"),
            platform: platform.clone(),
            surface: surface.clone(),
            direction: meeting_chat_direction(platform, parsed.sender.as_deref()),
            sender: parsed.sender,
            timestamp: parsed.timestamp,
            links: extract_links(&parsed.text),
            text: parsed.text,
        });
    }

    if messages.len() > 80 {
        messages.drain(..messages.len() - 80);
    }
    messages
}

#[cfg(target_os = "macos")]
fn meeting_chat_direction(
    platform: &MeetingPlatform,
    sender: Option<&str>,
) -> Option<MeetingChatDirection> {
    if *platform != MeetingPlatform::Zoom {
        return None;
    }

    sender.map(|sender| {
        let sender = sender.trim().to_lowercase();
        if matches!(sender.as_str(), "you" | "me") || sender.ends_with(" (you)") {
            MeetingChatDirection::Outgoing
        } else {
            MeetingChatDirection::Incoming
        }
    })
}

#[cfg(target_os = "macos")]
struct ParsedChatMessage {
    sender: Option<String>,
    timestamp: Option<String>,
    text: String,
}

#[cfg(target_os = "macos")]
fn chat_message_text(node: &AxNode) -> Option<String> {
    let role = node.role.as_deref().unwrap_or_default();
    if node.settable_value || matches!(role, "AXTextField" | "AXTextArea") {
        return None;
    }
    if candidate_chat_target(node).is_some_and(|target| {
        matches!(
            target.kind.as_str(),
            "input" | "sendButton" | "openChatControl"
        )
    }) {
        return None;
    }

    let value = node
        .value
        .as_deref()
        .or(node.title.as_deref())
        .or(node.description.as_deref())?;
    let text = normalize_chat_text(value);
    if text.len() < 2 || is_chat_chrome_text(&text) {
        return None;
    }

    Some(text)
}

#[cfg(target_os = "macos")]
fn parse_chat_message(platform: &MeetingPlatform, raw_text: &str) -> Option<ParsedChatMessage> {
    match platform {
        MeetingPlatform::Zoom => {
            parse_zoom_chat_message(raw_text).or_else(|| parse_web_chat_message(raw_text))
        }
        MeetingPlatform::Slack => {
            parse_slack_chat_message(raw_text).or_else(|| parse_web_chat_message(raw_text))
        }
        MeetingPlatform::GoogleMeet | MeetingPlatform::MicrosoftTeams | MeetingPlatform::Webex => {
            parse_web_chat_message(raw_text)
        }
        MeetingPlatform::Discord | MeetingPlatform::Unknown => None,
    }
}

#[cfg(target_os = "macos")]
fn parse_web_chat_message(raw_text: &str) -> Option<ParsedChatMessage> {
    let lines = chat_lines(raw_text);
    let first = lines.first()?.as_str();

    if lines.len() == 1 {
        let (sender_and_text, timestamp) = first.rsplit_once(", ")?;
        if !looks_like_time(timestamp) {
            return None;
        }
        let (sender, text) = sender_and_text.split_once(", ")?;
        let sender = sender.trim();
        let text = text.trim();
        return (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(timestamp.trim().to_string()),
                text: text.to_string(),
            });
    }

    if let Some((sender, timestamp)) = split_sender_time(first) {
        let text = lines[1..].join("\n").trim().to_string();
        return (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(timestamp.to_string()),
                text,
            });
    }

    if lines.len() >= 3 && looks_like_time(&lines[1]) {
        let sender = first.trim();
        let text = lines[2..].join("\n").trim().to_string();
        return (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(lines[1].clone()),
                text,
            });
    }

    let timestamp = lines.last()?;
    if lines.len() >= 3 && looks_like_time(timestamp) {
        let sender = first.trim();
        let text = lines[1..lines.len() - 1].join("\n").trim().to_string();
        return (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(timestamp.clone()),
                text,
            });
    }

    None
}

#[cfg(target_os = "macos")]
fn looks_like_chat_sender(sender: &str) -> bool {
    let sender = sender.trim();
    if sender.is_empty()
        || sender.chars().count() > 120
        || sender.contains('\n')
        || sender.contains('\r')
        || looks_like_time(sender)
        || is_chat_chrome_text(sender)
    {
        return false;
    }

    let lower = sender.to_ascii_lowercase();
    !matches!(
        lower.as_str(),
        "google meet" | "microsoft teams" | "zoom" | "slack" | "webex"
    ) && !lower.starts_with("recording ")
        && !lower.starts_with("meeting started")
        && !lower.starts_with("meeting ended")
}

#[cfg(target_os = "macos")]
fn parse_zoom_chat_message(raw_text: &str) -> Option<ParsedChatMessage> {
    let lines = chat_lines(raw_text);
    let first = lines.first()?.as_str();

    if lines.len() == 1 {
        let (sender, message_and_time) = first.split_once(", ")?;
        let (text, timestamp) = message_and_time.rsplit_once(", ")?;
        if looks_like_time(timestamp) {
            let text = text.trim();
            return (!sender.trim().is_empty() && !text.is_empty()).then(|| ParsedChatMessage {
                sender: non_empty_string(sender),
                timestamp: Some(timestamp.trim().to_string()),
                text: text.to_string(),
            });
        }
    }

    if !first.starts_with("From ") {
        return None;
    }

    let mut sender = first.trim_start_matches("From ").trim();
    if let Some((name, _target)) = sender.split_once(" to ") {
        sender = name.trim();
    }

    let mut timestamp = None;
    let mut message_start = 1;
    if let Some(line) = lines.get(1) {
        if looks_like_time(line) {
            timestamp = Some(line.clone());
            message_start = 2;
        }
    }

    let text = lines[message_start..].join("\n").trim().to_string();
    (!text.is_empty()).then(|| ParsedChatMessage {
        sender: non_empty_string(sender),
        timestamp,
        text,
    })
}

#[cfg(target_os = "macos")]
fn parse_slack_chat_message(raw_text: &str) -> Option<ParsedChatMessage> {
    let lines = chat_lines(raw_text);
    if lines.len() == 1 {
        return parse_slack_accessibility_description(&lines[0]);
    }

    if lines.len() < 2 {
        return None;
    }

    let first_line = lines[0].as_str();
    let (sender, timestamp, message_start) =
        if let Some((name, time)) = split_sender_time(first_line) {
            (name, time.to_string(), 1)
        } else if looks_like_time(&lines[1]) {
            (first_line, lines[1].clone(), 2)
        } else {
            return None;
        };

    let text = lines[message_start..].join("\n").trim().to_string();
    (!text.is_empty() && !is_chat_chrome_text(&text)).then(|| ParsedChatMessage {
        sender: non_empty_string(sender),
        timestamp: Some(timestamp),
        text,
    })
}

#[cfg(target_os = "macos")]
fn parse_slack_accessibility_description(line: &str) -> Option<ParsedChatMessage> {
    let line = line.trim().trim_end_matches('.');
    let (sender, message_and_time) = line.split_once(": ")?;

    for (separator, _) in message_and_time.rmatch_indices(". ") {
        let text = message_and_time[..separator].trim();
        let timestamp = message_and_time[separator + 2..].trim();
        let Some((date, time)) = timestamp.rsplit_once(" at ") else {
            continue;
        };

        if !sender.trim().is_empty()
            && !text.is_empty()
            && !date.trim().is_empty()
            && looks_like_time(time)
            && !is_chat_chrome_text(text)
        {
            return Some(ParsedChatMessage {
                sender: non_empty_string(sender),
                timestamp: Some(time.trim().to_string()),
                text: text.to_string(),
            });
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn chat_lines(text: &str) -> Vec<String> {
    normalize_chat_text(text)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(target_os = "macos")]
fn normalize_chat_text(text: &str) -> String {
    text.replace(['\u{00a0}', '\u{202f}'], " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(target_os = "macos")]
fn is_chat_chrome_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    matches!(
        lower.as_str(),
        "chat"
            | "meeting chat"
            | "send"
            | "send message"
            | "send a message"
            | "message everyone"
            | "type a message"
            | "conversation"
            | "message list"
            | "new messages"
    ) || lower.starts_with("type a message")
        || lower.starts_with("message everyone")
        || lower.starts_with("send a message")
}

#[cfg(target_os = "macos")]
fn split_sender_time(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim();
    for suffix in [" AM", " PM", " am", " pm"] {
        if let Some(without_period) = trimmed.strip_suffix(suffix) {
            let (name, clock) = without_period.rsplit_once(' ')?;
            let time_start = trimmed.len() - clock.len() - suffix.len();
            let time = &trimmed[time_start..];
            return looks_like_time(time).then_some((name.trim(), time.trim()));
        }
    }

    let (name, time) = trimmed.rsplit_once(' ')?;
    looks_like_time(time).then_some((name.trim(), time.trim()))
}

#[cfg(target_os = "macos")]
fn looks_like_time(text: &str) -> bool {
    let compact = text.trim().to_lowercase();
    let meridiem = compact
        .strip_suffix(" am")
        .or_else(|| compact.strip_suffix(" pm"));
    let time = meridiem.unwrap_or(&compact);
    let Some((hour, minute)) = time.split_once(':') else {
        return false;
    };

    let Ok(hour) = hour.parse::<u8>() else {
        return false;
    };
    let Ok(minute) = minute.parse::<u8>() else {
        return false;
    };

    minute < 60
        && if meridiem.is_some() {
            (1..=12).contains(&hour)
        } else {
            hour < 24
        }
}

#[cfg(target_os = "macos")]
fn non_empty_string(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(target_os = "macos")]
fn extract_links(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|part| {
            let link = part.trim_matches(|c: char| {
                matches!(
                    c,
                    '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | '.'
                )
            });
            (link.starts_with("http://") || link.starts_with("https://")).then(|| link.to_string())
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn candidate_chat_target(node: &AxNode) -> Option<MeetingChatTarget> {
    let role = node.role.as_deref().unwrap_or_default();
    let text = node.text.as_str();
    let mut confidence = 0.0;
    let mut signals = Vec::new();
    let mut kind = "unknown";

    let is_button = role == "AXButton" || role == "AXMenuItem";
    let is_send_button = text.contains("send") && is_button;
    let is_text_input = role == "AXTextArea" || role == "AXTextField";
    let has_chat_input_label = text.contains("send a message")
        || text.contains("message everyone")
        || text.contains("message to ")
        || text.contains("type a message")
        || text.contains("chat");
    let is_chat_control = is_button
        && !is_send_button
        && (text == "axbutton chat"
            || text == "axmenuitem chat"
            || text.contains("meeting chat")
            || text.contains("open chat")
            || text.contains("show chat")
            || text.contains("show/hide thread")
            || text.contains(" chat"));

    if is_text_input {
        confidence += 0.25;
        signals.push("text-input-role".to_string());
        kind = "input";
    }
    if has_chat_input_label {
        confidence += 0.4;
        signals.push("chat-label".to_string());
    }
    if is_chat_control {
        confidence += 0.45;
        signals.push("open-chat-control".to_string());
        kind = "openChatControl";
    }
    if is_send_button {
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

    if kind == "input" && (!is_text_input || !node.settable_value || !has_chat_input_label) {
        return None;
    }

    if confidence < 0.35 {
        return None;
    }

    Some(MeetingChatTarget {
        kind: kind.to_string(),
        #[cfg(test)]
        settable: node.settable_value,
        confidence,
        #[cfg(test)]
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
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

    #[test]
    fn test_slack_huddle_requires_huddle_label_and_enabled_leave_control() {
        let mut composer = node(2, "AXTextArea", "Message to test", None);
        composer.settable_value = true;
        let mut disabled_leave = node(1, "AXButton", "Leave Huddle", None);
        disabled_leave.enabled = Some(false);

        assert_eq!(slack_huddle_context(&[composer.clone()]), None);
        assert_eq!(
            slack_huddle_context(&[node(0, "AXWindow", "Huddle in test", None), disabled_leave,]),
            None
        );
        assert_eq!(
            slack_huddle_context(&[
                node(0, "AXWindow", "Huddle in  test", None),
                node(1, "AXButton", "Leave Huddle", None),
                composer,
            ]),
            Some(("Huddle in  test".to_string(), "test".to_string()))
        );
    }

    #[test]
    fn test_slack_live_huddle_controls_fit_tree_depth_budget() {
        assert!(MAX_TREE_DEPTH >= 14);
    }

    #[test]
    fn test_ordinary_slack_composer_is_not_a_huddle_composer() {
        let mut composer = node(2, "AXTextArea", "Message #general", None);
        composer.settable_value = true;

        assert!(!is_slack_huddle_composer(&composer, "test"));
        assert!(candidate_chat_target(&composer).is_none());
        assert_eq!(slack_huddle_context(&[composer]), None);
    }

    #[test]
    fn test_slack_hidden_thread_control_is_recognized() {
        let control = node(3, "AXButton", "Show/hide Thread", None);

        assert!(is_slack_thread_control(&control));
        assert_eq!(
            candidate_chat_target(&control).unwrap().kind,
            "openChatControl"
        );
    }

    #[test]
    fn test_slack_composer_and_send_button_must_share_live_thread_container() {
        let mut composer = node(4, "AXTextArea", "Message to test", None);
        composer.settable_value = true;
        let thread = [
            ancestor("Thread in test (private channel)"),
            ancestor("composer"),
        ];
        let other_thread = [ancestor("Thread in random (private channel)")];
        let duplicate_label_other_path = [ancestor_at("Thread in test (private channel)", &[9, 4])];

        assert!(is_slack_huddle_composer_in_thread(
            &composer, &thread, "test"
        ));
        assert!(!is_slack_huddle_composer_in_thread(&composer, &[], "test"));

        let mut send = node(5, "AXButton", "Send now", None);
        send.enabled = Some(false);
        assert!(!is_slack_send_now_in_thread(&send, &thread, "test", &[0]));

        send.enabled = Some(true);
        assert!(is_slack_send_now_in_thread(&send, &thread, "test", &[0]));
        assert!(!is_slack_send_now_in_thread(
            &send,
            &other_thread,
            "test",
            &[0]
        ));
        assert!(!is_slack_send_now_in_thread(
            &send,
            &duplicate_label_other_path,
            "test",
            &[0]
        ));
    }

    #[test]
    fn test_slack_composer_selection_fails_on_ambiguity_and_drafts() {
        let mut first = node(4, "AXTextArea", "Message to test", None);
        first.settable_value = true;
        let mut second = first.clone();
        second.index = 5;

        assert_eq!(
            unique_matching_index([&first, &second].into_iter().enumerate(), |node| {
                is_slack_huddle_composer(node, "test")
            },),
            UniqueMatch::Ambiguous
        );

        first.value = Some("existing draft".to_string());
        assert!(has_nonempty_draft(&first));
        first.value = Some(" \n ".to_string());
        assert!(!has_nonempty_draft(&first));
        assert!(chat_input_is_owned("disclosure", "disclosure"));
        assert!(!chat_input_is_owned(
            "disclosure plus user text",
            "disclosure"
        ));
    }

    #[test]
    fn test_chat_inspection_does_not_use_writable_value_as_label() {
        let mut input = node(6, "AXTextArea", "", None);
        input.title = None;
        input.settable_value = true;
        input.value = Some("private draft".to_string());
        input.text = searchable_node_text(
            &input.role,
            &input.title,
            &input.value,
            &input.description,
            &input.placeholder,
            input.settable_value,
        );

        assert!(!input.text.contains("private draft"));
        assert!(candidate_chat_target(&input).is_none());
        assert_eq!(inspection_label(&input), None);

        let mut read_only_input = input.clone();
        read_only_input.settable_value = false;
        read_only_input.value = Some("private read-only text".to_string());
        read_only_input.text = searchable_node_text(
            &read_only_input.role,
            &read_only_input.title,
            &read_only_input.value,
            &read_only_input.description,
            &read_only_input.placeholder,
            read_only_input.settable_value,
        );
        assert!(!read_only_input.text.contains("private read-only text"));
        assert_eq!(node_labels(&read_only_input).count(), 0);

        let mut secure_input = read_only_input.clone();
        secure_input.role = Some("AXSecureTextField".to_string());
        secure_input.value = Some("private password".to_string());
        secure_input.text = searchable_node_text(
            &secure_input.role,
            &secure_input.title,
            &secure_input.value,
            &secure_input.description,
            &secure_input.placeholder,
            secure_input.settable_value,
        );
        assert!(!secure_input.text.contains("private password"));
        assert_eq!(node_labels(&secure_input).count(), 0);
    }

    #[test]
    fn test_native_meeting_window_validation_is_evidence_backed() {
        let settings = [
            node(0, "AXWindow", "Zoom Workplace Settings", None),
            node(1, "AXStaticText", "Video", None),
            node(2, "AXButton", "Camera preview", None),
        ];
        let zoom_meeting = [node(
            3,
            "AXGroup",
            "Video render Ada Lovelace, Computer audio unmuted",
            None,
        )];
        let discord_voice = [node(4, "AXStaticText", "Voice connected", None)];

        assert!(!native_meeting_window_is_validated(
            &MeetingPlatform::Zoom,
            &settings,
        ));
        assert!(native_meeting_window_is_validated(
            &MeetingPlatform::Zoom,
            &zoom_meeting,
        ));
        assert!(native_meeting_window_is_validated(
            &MeetingPlatform::Discord,
            &discord_voice,
        ));
        for platform in [MeetingPlatform::MicrosoftTeams, MeetingPlatform::Webex] {
            assert!(!native_meeting_window_is_validated(
                &platform,
                &zoom_meeting,
            ));
        }
        assert!(native_meeting_window_is_validated(
            &MeetingPlatform::MicrosoftTeams,
            &[fixture_node(5, "AXButton", "Hang up", &[0])],
        ));
        assert!(native_meeting_window_is_validated(
            &MeetingPlatform::Webex,
            &[fixture_node(6, "AXButton", "Leave meeting", &[0])],
        ));
    }

    #[test]
    fn test_meeting_chat_message_validation() {
        assert!(validate_meeting_chat_message("disclosure message").is_ok());
        assert!(validate_meeting_chat_message(" \n\t ").is_err());
        assert!(validate_meeting_chat_message(&"x".repeat(2_000)).is_ok());
        assert!(validate_meeting_chat_message(&"x".repeat(2_001)).is_err());
    }

    #[test]
    fn test_chat_mutation_is_fail_closed_for_unvalidated_platforms() {
        assert!(supports_meeting_chat_mutation("com.tinyspeck.slackmacgap"));
        assert!(supports_meeting_chat_mutation("com.slack.Slack"));
        for bundle_id in [
            "us.zoom.xos",
            "com.microsoft.teams2",
            "com.hnc.Discord",
            "Cisco-Systems.Spark",
            "com.google.Chrome",
        ] {
            assert!(!supports_meeting_chat_mutation(bundle_id));
        }
    }

    #[test]
    fn test_established_native_bundle_aliases_are_classified() {
        for (bundle_id, platform) in [
            ("com.slack.Slack", MeetingPlatform::Slack),
            ("com.cisco.webex", MeetingPlatform::Webex),
            ("com.cisco.webexmeetingsapp", MeetingPlatform::Webex),
            ("com.discordapp.Discord", MeetingPlatform::Discord),
        ] {
            assert!(is_meeting_app_bundle(bundle_id));
            assert_eq!(classify_bundle(bundle_id), platform);
            assert_eq!(
                classify_surface(bundle_id, &platform),
                MeetingSurface::Native
            );
        }
    }

    #[test]
    fn test_established_browser_variants_are_recognized_as_web_surfaces() {
        for bundle_id in [
            "com.apple.SafariTechnologyPreview",
            "com.google.Chrome.canary",
            "com.microsoft.edgemac.Beta",
            "com.microsoft.edgemac.Canary",
            "com.microsoft.edgemac.Dev",
            "org.mozilla.firefoxdeveloperedition",
            "org.mozilla.nightly",
            "com.brave.Browser.beta",
            "com.brave.Browser.nightly",
            "org.chromium.Chromium",
            "com.operasoftware.OperaDeveloper",
            "com.operasoftware.OperaGX",
            "com.operasoftware.OperaNext",
            "net.imput.helium",
        ] {
            assert!(is_meeting_app_bundle(bundle_id));
            assert!(is_browser_bundle(bundle_id));
            assert_eq!(
                classify_surface(bundle_id, &MeetingPlatform::Unknown),
                MeetingSurface::Web
            );
        }
    }

    #[test]
    fn test_meeting_app_registry_drives_bundle_kind() {
        let mut seen = HashSet::new();

        for bundle in MEETING_APP_BUNDLES {
            assert!(seen.insert(bundle.id), "duplicate bundle id: {}", bundle.id);
            assert!(is_meeting_app_bundle(bundle.id));
            assert_eq!(
                is_browser_bundle(bundle.id),
                bundle.kind == MeetingAppBundleKind::Browser,
                "unexpected browser classification for {}",
                bundle.id
            );
        }
    }

    #[test]
    fn test_chat_mutation_scope_deduplicates_one_recognized_meeting_app() {
        let bundle_ids = vec![
            "com.tinyspeck.slackmacgap".to_string(),
            "com.tinyspeck.slackmacgap".to_string(),
            "com.hyprnote.dev".to_string(),
        ];

        assert_eq!(
            unique_recognized_meeting_bundle(&bundle_ids),
            Ok("com.tinyspeck.slackmacgap")
        );
    }

    #[test]
    fn test_chat_mutation_scope_rejects_zero_or_multiple_meeting_apps() {
        assert!(unique_recognized_meeting_bundle(&[]).is_err());
        assert!(
            unique_recognized_meeting_bundle(&[
                "us.zoom.xos".to_string(),
                "com.tinyspeck.slackmacgap".to_string(),
            ])
            .is_err()
        );
    }

    #[test]
    fn test_zoom_scope_does_not_fall_back_to_an_unrelated_slack_huddle() {
        let bundle_ids = ["us.zoom.xos".to_string()];
        let scoped_bundle = unique_recognized_meeting_bundle(&bundle_ids).unwrap();

        assert_eq!(scoped_bundle, "us.zoom.xos");
        assert!(!supports_meeting_chat_mutation(scoped_bundle));
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
    fn test_visible_empty_chat_surface_can_establish_a_capture_baseline() {
        let mut zoom_chat_list = node(4, "AXTable", "Chat list", None);
        zoom_chat_list.within_zoom_meeting_scope = true;
        zoom_chat_list.within_zoom_chat_scope = true;
        assert!(meeting_chat_surface_is_visible(
            &MeetingPlatform::Zoom,
            &[zoom_chat_list],
        ));

        let mut zoom_rename_input = node(5, "AXTextField", "Display name", None);
        zoom_rename_input.settable_value = true;
        zoom_rename_input.within_zoom_meeting_scope = true;
        zoom_rename_input.text = node_text(
            &zoom_rename_input.role,
            &zoom_rename_input.title,
            &zoom_rename_input.value,
            &zoom_rename_input.description,
            &zoom_rename_input.placeholder,
        );
        assert!(!meeting_chat_surface_is_visible(
            &MeetingPlatform::Zoom,
            &[zoom_rename_input],
        ));

        let mut slack_input = node(6, "AXTextArea", "Message to test", None);
        slack_input.settable_value = true;
        slack_input.within_slack_huddle_scope = true;
        slack_input.text = node_text(
            &slack_input.role,
            &slack_input.title,
            &slack_input.value,
            &slack_input.description,
            &slack_input.placeholder,
        );
        assert!(meeting_chat_surface_is_visible(
            &MeetingPlatform::Slack,
            &[slack_input],
        ));

        assert!(!meeting_chat_surface_is_visible(
            &MeetingPlatform::Slack,
            &[node(7, "AXTextArea", "Search", None)],
        ));
    }

    #[test]
    fn test_browser_chat_scope_requires_live_exit_visible_composer_and_platform_container() {
        let meet_nodes = vec![
            fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
            fixture_node(1, "AXButton", "Leave call", &[0]),
            fixture_node(2, "AXGroup", "In-call messages", &[1]),
            fixture_composer(3, "Send a message", &[1, 0]),
        ];

        assert_eq!(
            validated_chat_scope(&MeetingPlatform::GoogleMeet, &meet_nodes),
            Some((vec![1], vec![1, 0]))
        );

        let mut prejoin_nodes = meet_nodes.clone();
        prejoin_nodes[1] = fixture_node(1, "AXButton", "Turn off microphone", &[0]);
        assert!(validated_chat_scope(&MeetingPlatform::GoogleMeet, &prejoin_nodes).is_none());

        let mut hidden_composer_nodes = meet_nodes.clone();
        hidden_composer_nodes[3].bounds = None;
        assert!(
            validated_chat_scope(&MeetingPlatform::GoogleMeet, &hidden_composer_nodes).is_none()
        );

        let mut duplicate_composer_nodes = meet_nodes.clone();
        duplicate_composer_nodes.push(fixture_composer(4, "Send a message", &[1, 1]));
        assert!(
            validated_chat_scope(&MeetingPlatform::GoogleMeet, &duplicate_composer_nodes).is_none()
        );

        let mut support_widget_nodes = meet_nodes.clone();
        support_widget_nodes[2] = fixture_node(2, "AXGroup", "Support chat", &[1]);
        assert!(
            validated_chat_scope(&MeetingPlatform::GoogleMeet, &support_widget_nodes).is_none()
        );
    }

    #[test]
    fn test_platform_chat_adapters_validate_the_requested_provider_matrix() {
        for (platform, exit_label, scope_label, composer_label) in [
            (
                MeetingPlatform::GoogleMeet,
                "Leave call",
                "In-call messages",
                "Send a message",
            ),
            (
                MeetingPlatform::MicrosoftTeams,
                "Hang up",
                "Meeting chat",
                "Type a message",
            ),
            (
                MeetingPlatform::Zoom,
                "Leave meeting",
                "Chat",
                "Message everyone",
            ),
            (
                MeetingPlatform::Webex,
                "Leave meeting",
                "Chat with everyone",
                "Type a message",
            ),
        ] {
            let nodes = vec![
                fixture_node(0, "AXWebArea", "Meeting", &[]),
                fixture_node(1, "AXButton", exit_label, &[0]),
                fixture_node(2, "AXGroup", scope_label, &[1]),
                fixture_composer(3, composer_label, &[1, 0]),
            ];

            assert_eq!(
                validated_chat_scope(&platform, &nodes),
                Some((vec![1], vec![1, 0])),
                "adapter did not validate {platform:?}"
            );
        }

        let slack_nodes = vec![
            fixture_node(0, "AXWebArea", "Huddle in test", &[]),
            fixture_node(1, "AXButton", "Leave huddle", &[0]),
            fixture_node(2, "AXGroup", "Thread in test", &[1]),
            fixture_composer(3, "Message to test", &[1, 0]),
        ];
        assert_eq!(
            validated_chat_scope(&MeetingPlatform::Slack, &slack_nodes),
            Some((vec![1], vec![1, 0]))
        );

        let mut ordinary_channel_nodes = slack_nodes;
        ordinary_channel_nodes[2] = fixture_node(2, "AXGroup", "Channel test", &[1]);
        assert!(validated_chat_scope(&MeetingPlatform::Slack, &ordinary_channel_nodes).is_none());
    }

    #[test]
    fn test_teams_and_webex_reject_generic_chat_containers() {
        for (platform, exit_label, composer_label) in [
            (MeetingPlatform::MicrosoftTeams, "Hang up", "Type a message"),
            (MeetingPlatform::Webex, "Leave meeting", "Send a message"),
        ] {
            let nodes = vec![
                fixture_node(0, "AXWebArea", "Meeting", &[]),
                fixture_node(1, "AXButton", exit_label, &[0]),
                fixture_node(2, "AXGroup", "Chat", &[1]),
                fixture_composer(3, composer_label, &[1, 0]),
            ];

            assert!(
                validated_chat_scope(&platform, &nodes).is_none(),
                "generic chat unexpectedly validated for {platform:?}"
            );
        }
    }

    #[test]
    fn test_web_capture_extracts_only_the_validated_chat_subtree() {
        let nodes = vec![
            fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
            fixture_node(1, "AXButton", "Leave call", &[0]),
            fixture_node(2, "AXGroup", "In-call messages", &[1]),
            fixture_composer(3, "Send a message", &[1, 0]),
            fixture_node(
                4,
                "AXGroup",
                "Ada Lovelace\n10:42 AM\nDiscuss the rollout https://example.com/plan",
                &[1, 1],
            ),
            fixture_node(
                5,
                "AXGroup",
                "Mallory\n10:43 AM\nUnrelated browser content",
                &[2, 0],
            ),
        ];

        let messages =
            extract_chat_messages(&MeetingPlatform::GoogleMeet, &MeetingSurface::Web, &nodes);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender.as_deref(), Some("Ada Lovelace"));
        assert_eq!(messages[0].timestamp.as_deref(), Some("10:42 AM"));
        assert_eq!(
            messages[0].text,
            "Discuss the rollout https://example.com/plan"
        );
        assert_eq!(messages[0].links, vec!["https://example.com/plan"]);
    }

    #[test]
    fn test_web_capture_ignores_aggregate_containers() {
        let mut aggregate_list = fixture_node(4, "AXList", "Message list", &[1, 1]);
        aggregate_list.value = Some("Mallory\n10:41 AM\nAggregated list value".to_string());
        let nodes = vec![
            fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
            fixture_node(1, "AXButton", "Leave call", &[0]),
            fixture_node(2, "AXGroup", "In-call messages", &[1]),
            fixture_composer(3, "Send a message", &[1, 0]),
            aggregate_list,
            fixture_node(
                5,
                "AXGroup",
                "Ada Lovelace\n10:42 AM\nFirst message\nGrace Hopper\n10:43 AM\nSecond message",
                &[1, 2],
            ),
            fixture_node(
                6,
                "AXGroup",
                "Ada Lovelace\n10:42 AM\nFirst message",
                &[1, 2, 0],
            ),
            fixture_node(
                7,
                "AXGroup",
                "Grace Hopper\n10:43 AM\nSecond message",
                &[1, 2, 1],
            ),
        ];

        let messages =
            extract_chat_messages(&MeetingPlatform::GoogleMeet, &MeetingSurface::Web, &nodes);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "First message");
        assert_eq!(messages[1].text, "Second message");
    }

    #[test]
    fn test_browser_context_preserves_query_identified_meeting_identity() {
        let teams_nodes = vec![
            fixture_node(0, "AXWebArea", "Microsoft Teams", &[]),
            fixture_node(1, "AXButton", "Hang up", &[0]),
            fixture_node(2, "AXGroup", "Meeting chat", &[1]),
            fixture_composer(3, "Type a message", &[1, 0]),
        ];
        let root = |url: &str| BrowserMeetingRoot {
            platform: MeetingPlatform::MicrosoftTeams,
            window_title: Some("Microsoft Teams meeting".to_string()),
            web_area_url: Some(url.to_string()),
            nodes: teams_nodes.clone(),
        };

        let first = browser_capture_context_id(&root(
            "https://teams.microsoft.com/v2/?meetingId=first#fragment",
        ))
        .unwrap();
        let same_without_fragment =
            browser_capture_context_id(&root("https://teams.microsoft.com/v2/?meetingId=first"))
                .unwrap();
        let second =
            browser_capture_context_id(&root("https://teams.microsoft.com/v2/?meetingId=second"))
                .unwrap();

        assert_eq!(first, same_without_fragment);
        assert_ne!(first, second);
    }

    #[test]
    fn test_capture_context_ignores_volatile_titles_and_tree_paths() {
        let nodes = |scope_path: &[usize], composer_path: &[usize], root_role: &str| {
            vec![
                fixture_node(0, root_role, "Microsoft Teams", &[]),
                fixture_node(1, "AXButton", "Hang up", &[0]),
                fixture_node(2, "AXGroup", "Meeting chat", scope_path),
                fixture_composer(3, "Type a message", composer_path),
            ]
        };
        let browser_root =
            |title: &str, scope_path: &[usize], composer_path: &[usize]| BrowserMeetingRoot {
                platform: MeetingPlatform::MicrosoftTeams,
                window_title: Some(title.to_string()),
                web_area_url: Some("https://teams.microsoft.com/v2/?meetingId=stable".to_string()),
                nodes: nodes(scope_path, composer_path, "AXWebArea"),
            };

        let first =
            browser_capture_context_id(&browser_root("Microsoft Teams meeting", &[1], &[1, 0]))
                .unwrap();
        let shifted =
            browser_capture_context_id(&browser_root("(1) Microsoft Teams meeting", &[5], &[5, 2]))
                .unwrap();

        assert_eq!(first, shifted);

        let first_native = NativeMeetingRoot {
            window_title: Some("Microsoft Teams meeting".to_string()),
            nodes: nodes(&[1], &[1, 0], "AXWindow"),
        };
        let shifted_native = NativeMeetingRoot {
            window_title: Some("Microsoft Teams meeting · new activity".to_string()),
            nodes: nodes(&[5], &[5, 2], "AXWindow"),
        };

        assert_eq!(
            native_capture_context_id(&MeetingPlatform::MicrosoftTeams, &first_native),
            native_capture_context_id(&MeetingPlatform::MicrosoftTeams, &shifted_native)
        );
    }

    #[test]
    fn test_chat_button_is_open_chat_control_not_input() {
        let target = candidate_chat_target(&node(4, "AXButton", "Chat", None)).unwrap();

        assert_eq!(target.kind, "openChatControl");
        assert!(!target.settable);
        assert!(target.signals.contains(&"open-chat-control".to_string()));
    }

    #[test]
    fn test_zoom_chat_message_parser_preserves_sender_time_text_and_links() {
        let parsed = parse_chat_message(
            &MeetingPlatform::Zoom,
            "From Ada Lovelace to Everyone\n10:42 AM\nHere is the doc https://example.com/spec.",
        )
        .unwrap();

        assert_eq!(parsed.sender, Some("Ada Lovelace".to_string()));
        assert_eq!(parsed.timestamp, Some("10:42 AM".to_string()));
        assert_eq!(parsed.text, "Here is the doc https://example.com/spec.");
        assert_eq!(
            extract_links(&parsed.text),
            vec!["https://example.com/spec"]
        );
    }

    #[test]
    fn test_zoom_chat_message_parser_handles_current_native_row_description() {
        let parsed = parse_chat_message(
            &MeetingPlatform::Zoom,
            "You, ANLG-76 AX integration 5844 https://example.com/ax-test, 4:16\u{202f}PM",
        )
        .unwrap();

        assert_eq!(parsed.sender, Some("You".to_string()));
        assert_eq!(parsed.timestamp, Some("4:16 PM".to_string()));
        assert_eq!(
            parsed.text,
            "ANLG-76 AX integration 5844 https://example.com/ax-test"
        );
        assert_eq!(
            extract_links(&parsed.text),
            vec!["https://example.com/ax-test"]
        );
    }

    #[test]
    fn test_zoom_chat_direction_uses_native_self_sender_label() {
        assert_eq!(
            meeting_chat_direction(&MeetingPlatform::Zoom, Some("You")),
            Some(MeetingChatDirection::Outgoing)
        );
        assert_eq!(
            meeting_chat_direction(&MeetingPlatform::Zoom, Some("Ada")),
            Some(MeetingChatDirection::Incoming)
        );
        assert_eq!(
            meeting_chat_direction(&MeetingPlatform::Slack, Some("You")),
            None
        );
    }

    #[test]
    fn test_zoom_capture_requires_zoom_meeting_window_scope() {
        assert!(is_zoom_meeting_scope_node(&node(
            0,
            "AXWindow",
            "Zoom Meeting",
            None,
        )));
        assert!(is_zoom_meeting_scope_node(&node(
            1,
            "AXWindow",
            "John Jeong's Zoom Meeting",
            None,
        )));
        assert!(!is_zoom_meeting_scope_node(&node(
            2,
            "AXWindow",
            "Zoom Workplace",
            None,
        )));

        let mut chat_row = node(3, "AXGroup", "You, meeting chat message, 4:16 PM", None);
        chat_row.identifier = Some("ZMTextMessageCellView".to_string());
        assert!(is_zoom_chat_scope_node(&chat_row));

        let mut meeting_caption = node(
            4,
            "AXStaticText",
            "Ada, confidential caption, 4:16 PM",
            None,
        );
        meeting_caption.within_zoom_meeting_scope = true;
        assert!(
            extract_chat_messages(
                &MeetingPlatform::Zoom,
                &MeetingSurface::Native,
                &[meeting_caption],
            )
            .is_empty()
        );

        let team_chat_message = node(3, "AXStaticText", "You, private team chat, 4:16 PM", None);
        assert!(
            extract_chat_messages(
                &MeetingPlatform::Zoom,
                &MeetingSurface::Native,
                &[team_chat_message],
            )
            .is_empty()
        );
    }

    #[test]
    fn test_slack_chat_message_parser_handles_sender_time_prefix() {
        let parsed = parse_chat_message(
            &MeetingPlatform::Slack,
            "Grace Hopper 9:03 PM\nShip it after the final check",
        )
        .unwrap();

        assert_eq!(parsed.sender, Some("Grace Hopper".to_string()));
        assert_eq!(parsed.timestamp, Some("9:03 PM".to_string()));
        assert_eq!(parsed.text, "Ship it after the final check");
    }

    #[test]
    fn test_slack_chat_message_parser_handles_native_accessibility_description() {
        let parsed = parse_chat_message(
            &MeetingPlatform::Slack,
            "John Jeong: @Artem lorem ipsum. Friday at 5:50\u{202f}PM.",
        )
        .unwrap();

        assert_eq!(parsed.sender, Some("John Jeong".to_string()));
        assert_eq!(parsed.timestamp, Some("5:50 PM".to_string()));
        assert_eq!(parsed.text, "@Artem lorem ipsum");
    }

    #[test]
    fn test_web_chat_parsers_cover_meet_teams_zoom_slack_and_webex_shapes() {
        for (platform, raw_text, sender, timestamp, text) in [
            (
                MeetingPlatform::GoogleMeet,
                "Ada Lovelace\n10:42 AM\nMeet decision",
                "Ada Lovelace",
                "10:42 AM",
                "Meet decision",
            ),
            (
                MeetingPlatform::MicrosoftTeams,
                "Grace Hopper 10:43 AM\nTeams decision",
                "Grace Hopper",
                "10:43 AM",
                "Teams decision",
            ),
            (
                MeetingPlatform::Zoom,
                "Linus Torvalds, Zoom decision, 10:44 AM",
                "Linus Torvalds",
                "10:44 AM",
                "Zoom decision",
            ),
            (
                MeetingPlatform::Slack,
                "Margaret Hamilton\nSlack decision\n10:45 AM",
                "Margaret Hamilton",
                "10:45 AM",
                "Slack decision",
            ),
            (
                MeetingPlatform::Webex,
                "Katherine Johnson\n10:46 AM\nWebex decision",
                "Katherine Johnson",
                "10:46 AM",
                "Webex decision",
            ),
        ] {
            let parsed = parse_chat_message(&platform, raw_text)
                .unwrap_or_else(|| panic!("failed to parse {platform:?}"));
            assert_eq!(parsed.sender.as_deref(), Some(sender));
            assert_eq!(parsed.timestamp.as_deref(), Some(timestamp));
            assert_eq!(parsed.text, text);
        }
    }

    #[test]
    fn test_past_slack_huddle_thread_is_not_captured_without_active_huddle() {
        let mut message = node(
            0,
            "AXGroup",
            "John Jeong: @Artem lorem ipsum. Friday at 5:50 PM.",
            None,
        );
        message.within_slack_huddle_scope = true;

        assert!(
            extract_chat_messages(&MeetingPlatform::Slack, &MeetingSurface::Native, &[message],)
                .is_empty()
        );
    }

    #[test]
    fn test_chat_parsers_reject_unstructured_static_text() {
        assert!(
            parse_chat_message(
                &MeetingPlatform::Zoom,
                "Recording has started for this meeting"
            )
            .is_none()
        );
        assert!(parse_chat_message(&MeetingPlatform::Slack, "Channels\nGeneral").is_none());
    }

    #[test]
    fn test_chat_parsers_reject_invalid_timestamps() {
        assert!(!looks_like_time("99:99"));
        assert!(!looks_like_time("13:00 PM"));
        assert!(looks_like_time("23:59"));
        assert!(looks_like_time("12:59 PM"));
    }

    #[test]
    fn test_active_bundle_selection_is_scoped_and_deduplicated() {
        let active_bundle_ids = vec![
            "com.tinyspeck.slackmacgap".to_string(),
            "com.google.Chrome".to_string(),
            "com.tinyspeck.slackmacgap".to_string(),
            "com.example.unrelated".to_string(),
        ];

        assert_eq!(
            select_active_bundle_ids(
                MEETING_APP_BUNDLES.iter().map(|bundle| bundle.id),
                &active_bundle_ids,
            ),
            vec!["com.tinyspeck.slackmacgap", "com.google.Chrome"]
        );
        assert!(
            select_active_bundle_ids(MEETING_APP_BUNDLES.iter().map(|bundle| bundle.id), &[],)
                .is_empty()
        );
    }

    #[test]
    fn test_slack_capture_requires_active_huddle_and_huddle_specific_scope() {
        let active_control = node(0, "AXButton", "Leave huddle", None);
        let channel_message = node(
            1,
            "AXStaticText",
            "Grace Hopper 9:03 PM\nChannel-only message",
            None,
        );

        assert!(
            extract_chat_messages(
                &MeetingPlatform::Slack,
                &MeetingSurface::Native,
                &[active_control.clone(), channel_message.clone()],
            )
            .is_empty()
        );

        let mut huddle_message = channel_message;
        huddle_message.within_slack_huddle_scope = true;
        assert!(
            extract_chat_messages(
                &MeetingPlatform::Slack,
                &MeetingSurface::Native,
                &[huddle_message.clone()],
            )
            .is_empty()
        );

        let messages = extract_chat_messages(
            &MeetingPlatform::Slack,
            &MeetingSurface::Native,
            &[active_control, huddle_message],
        );
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "Channel-only message");
    }

    #[test]
    fn test_slack_huddle_scope_rejects_general_channel_containers() {
        assert!(is_slack_huddle_scope_node(&node(
            0,
            "AXGroup",
            "Huddle chat",
            None,
        )));
        assert!(!is_slack_huddle_scope_node(&node(
            1,
            "AXGroup",
            "#general conversation",
            None,
        )));
        assert!(!slack_huddle_is_active(&[node(
            2,
            "AXStaticText",
            "Someone mentioned leave huddle in a channel message",
            None,
        )]));
    }

    #[test]
    fn test_slack_capture_context_identity_tracks_validated_surface() {
        let context = slack_capture_context_id("test", "Huddle in test", 0x101, 0x202);

        assert_eq!(
            context,
            slack_capture_context_id("test", "Huddle in test", 0x101, 0x202)
        );
        assert_ne!(
            context,
            slack_capture_context_id("another", "Huddle in another", 0x101, 0x202)
        );
        assert_ne!(
            context,
            slack_capture_context_id("test", "Huddle in test", 0x303, 0x202)
        );
        assert_ne!(
            context,
            slack_capture_context_id("test", "Huddle in test", 0x101, 0x404)
        );
    }

    #[test]
    fn test_zoom_capture_context_stays_stable_across_participant_changes() {
        let root = |chat_hash, participant_names: &[&str]| {
            let mut window = node(0, "AXWindow", "Zoom Meeting", None);
            window.element_hash = Some(0x101);
            window.within_zoom_meeting_scope = true;

            let mut chat = node(1, "AXTable", "Chat list", None);
            chat.element_hash = Some(chat_hash);
            chat.within_zoom_meeting_scope = true;
            chat.within_zoom_chat_scope = true;

            let participants = participant_names.iter().enumerate().map(|(index, name)| {
                let mut participant = node(
                    index + 2,
                    "AXGroup",
                    &format!("Video render {name}, Computer audio unmuted"),
                    None,
                );
                participant.element_hash = Some(0x300 + index);
                participant.within_zoom_meeting_scope = true;
                participant
            });

            NativeMeetingRoot {
                window_title: Some("Zoom Meeting".to_string()),
                nodes: std::iter::once(window)
                    .chain(std::iter::once(chat))
                    .chain(participants)
                    .collect(),
            }
        };

        let first = zoom_capture_context_id(&root(0x202, &["Ada", "Grace"])).unwrap();
        let reordered = zoom_capture_context_id(&root(0x202, &["Grace", "Ada"])).unwrap();
        let switched = zoom_capture_context_id(&root(0x202, &["Ada", "Linus"])).unwrap();
        let new_chat_surface = zoom_capture_context_id(&root(0x404, &["Ada", "Grace"])).unwrap();

        assert_eq!(first, reordered);
        assert_eq!(first, switched);
        assert_ne!(first, new_chat_surface);
    }

    #[test]
    fn test_extract_chat_messages_keeps_repeated_source_rows_distinct() {
        let message = "From Ada Lovelace to Everyone\n10:42 AM\nDecision: keep the launch date";
        let nodes = vec![
            zoom_message_node(12, message),
            zoom_message_node(13, message),
        ];

        let messages =
            extract_chat_messages(&MeetingPlatform::Zoom, &MeetingSurface::Native, &nodes);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].sender, Some("Ada Lovelace".to_string()));
        assert_eq!(messages[0].text, "Decision: keep the launch date");
        assert_ne!(messages[0].id, messages[1].id);
        assert!(messages[0].id.ends_with("occurrence=1"));
        assert!(messages[1].id.ends_with("occurrence=2"));
    }

    #[test]
    fn test_element_hash_stabilizes_identical_rows_across_snapshot_shifts() {
        let message = "From Ada Lovelace to Everyone\n10:42 AM\nDecision: keep the launch date";
        let hashed_node = |index, element_hash| {
            let mut node = zoom_message_node(index, message);
            node.element_hash = Some(element_hash);
            node
        };

        let first = extract_chat_messages(
            &MeetingPlatform::Zoom,
            &MeetingSurface::Native,
            &[hashed_node(0, 0x101), hashed_node(1, 0x202)],
        );
        let shifted = extract_chat_messages(
            &MeetingPlatform::Zoom,
            &MeetingSurface::Native,
            &[
                hashed_node(0, 0x303),
                hashed_node(1, 0x101),
                hashed_node(2, 0x202),
            ],
        );

        assert_eq!(first[0].id, shifted[1].id);
        assert_eq!(first[1].id, shifted[2].id);
        assert!(first[0].id.contains("cfhash=101"));
        assert!(first[0].id.contains("Decision: keep the launch date"));
    }

    #[test]
    fn test_extract_chat_messages_retains_newest_eighty_rows() {
        let nodes = (0..85)
            .map(|index| {
                zoom_message_node(index, &format!("From Ada to Everyone\nmessage {index}"))
            })
            .collect::<Vec<_>>();

        let messages =
            extract_chat_messages(&MeetingPlatform::Zoom, &MeetingSurface::Native, &nodes);

        assert_eq!(messages.len(), 80);
        assert_eq!(messages.first().unwrap().text, "message 5");
        assert_eq!(messages.last().unwrap().text, "message 84");
    }

    #[test]
    fn test_browser_title_classifies_meet_web() {
        let web_area = node(16, "AXWebArea", "Team sync - Google Meet", None);
        assert_eq!(
            classify_browser_context(
                Some("https://meet.google.com/abc-defg-hij"),
                Some("Team sync - Google Meet - Google Chrome"),
                Some(&web_area),
                &[],
            ),
            MeetingPlatform::GoogleMeet
        );
        assert_eq!(
            classify_surface("com.google.Chrome", &MeetingPlatform::GoogleMeet),
            MeetingSurface::Web
        );
    }

    #[test]
    fn test_browser_active_web_area_can_validate_one_platform_but_not_conflicts() {
        let meet_web_area = node(18, "AXWebArea", "Team sync - Google Meet", None);
        let generic_web_area = node(19, "AXWebArea", "Document", None);
        assert_eq!(
            classify_browser_context(
                Some("https://meet.google.com/abc-defg-hij"),
                Some("Google Chrome"),
                Some(&meet_web_area),
                &[],
            ),
            MeetingPlatform::GoogleMeet
        );

        assert_eq!(
            classify_browser_context(
                Some("https://meet.google.com/abc-defg-hij"),
                Some("Zoom Meeting - Google Chrome"),
                Some(&meet_web_area),
                &[],
            ),
            MeetingPlatform::Unknown
        );

        assert_eq!(
            classify_browser_context(
                Some("https://meet.google.com/abc-defg-hij"),
                Some("Google Chrome"),
                Some(&generic_web_area),
                &[],
            ),
            MeetingPlatform::Unknown
        );
        assert_eq!(
            classify_browser_context(
                Some("https://meet.google.com/abc-defg-hij"),
                Some("Google Chrome"),
                Some(&generic_web_area),
                &[node(20, "AXButton", "Leave call", None)],
            ),
            MeetingPlatform::GoogleMeet
        );

        assert_eq!(
            classify_browser_context(
                Some("https://www.google.com/search?q=Google+Meet"),
                Some("Google Meet - Google Search"),
                Some(&meet_web_area),
                &[],
            ),
            MeetingPlatform::Unknown
        );
    }

    #[test]
    fn test_browser_meeting_origins_are_exact_and_https_only() {
        for (url, platform) in [
            (
                "https://meet.google.com/abc-defg-hij",
                MeetingPlatform::GoogleMeet,
            ),
            (
                "https://teams.microsoft.com/v2/",
                MeetingPlatform::MicrosoftTeams,
            ),
            (
                "https://teams.live.com/meet/123",
                MeetingPlatform::MicrosoftTeams,
            ),
            ("https://app.zoom.us/wc/123", MeetingPlatform::Zoom),
            (
                "https://fastrepl.webex.com/meet/test",
                MeetingPlatform::Webex,
            ),
            (
                "https://app.slack.com/client/workspace/channel",
                MeetingPlatform::Slack,
            ),
        ] {
            assert_eq!(browser_platform_from_url(Some(url)), Some(platform));
        }

        for url in [
            "http://meet.google.com/abc-defg-hij",
            "https://meet.google.com.evil.example/abc-defg-hij",
            "https://teams.microsoft.com.evil.example/v2/",
            "https://zoom.us.evil.example/wc/123",
            "https://webex.com.evil.example/meet/test",
            "https://slack.com.evil.example/client/workspace/channel",
            "javascript:alert(1)",
        ] {
            assert_eq!(browser_platform_from_url(Some(url)), None, "accepted {url}");
        }
    }

    #[test]
    fn test_meet_chat_scope_accepts_chromium_webkit_and_gecko_role_variants() {
        for (container_role, composer_role) in [
            ("AXGroup", "AXTextArea"),
            ("AXScrollArea", "AXTextField"),
            ("AXList", "AXTextArea"),
        ] {
            let mut composer = fixture_composer(3, "Send a message", &[1, 0]);
            composer.role = Some(composer_role.to_string());
            let nodes = vec![
                fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
                fixture_node(1, "AXButton", "Leave call", &[0]),
                fixture_node(2, container_role, "In-call messages", &[1]),
                composer,
            ];

            assert_eq!(
                validated_chat_scope(&MeetingPlatform::GoogleMeet, &nodes),
                Some((vec![1], vec![1, 0]))
            );
        }
    }

    #[test]
    fn test_browser_meeting_window_scope_must_be_unique() {
        assert_eq!(unique_scope_for_count(0), UniqueMatch::Missing);
        assert_eq!(unique_scope_for_count(1), UniqueMatch::One(0));
        assert_eq!(unique_scope_for_count(2), UniqueMatch::Ambiguous);
        assert_eq!(unique_scope_for_search(1, true), UniqueMatch::One(0));
        assert_eq!(unique_scope_for_search(1, false), UniqueMatch::Ambiguous);
    }

    #[test]
    fn test_webex_native_bundle_classifies_native() {
        assert_eq!(
            classify_bundle("Cisco-Systems.Spark"),
            MeetingPlatform::Webex
        );
        assert_eq!(
            classify_surface("Cisco-Systems.Spark", &MeetingPlatform::Webex),
            MeetingSurface::Native
        );
    }

    #[test]
    fn test_webex_browser_title_classifies_web() {
        let web_area = node(21, "AXWebArea", "Cisco Webex Meetings", None);
        assert_eq!(
            classify_browser_context(
                Some("https://fastrepl.webex.com/meet/team"),
                Some("Cisco Webex Meetings - Brave Browser"),
                Some(&web_area),
                &[],
            ),
            MeetingPlatform::Webex
        );
        assert_eq!(
            classify_surface("com.brave.Browser", &MeetingPlatform::Webex),
            MeetingSurface::Web
        );
    }

    #[test]
    fn test_only_provider_like_browser_windows_poison_incomplete_capture() {
        assert!(!browser_window_has_provider_signal(
            Some("https://mail.google.com/mail/u/0/#inbox"),
            Some("Inbox - Gmail"),
        ));
        assert!(browser_window_has_provider_signal(
            Some("https://meet.google.com/abc-defg-hij"),
            Some("Weekly planning - Google Meet"),
        ));
        assert!(browser_window_has_provider_signal(
            None,
            Some("Team sync | Microsoft Teams"),
        ));
    }

    #[test]
    fn test_validated_browser_bundles_are_web_surfaces() {
        for bundle_id in [
            "com.google.Chrome",
            "com.microsoft.edgemac",
            "org.mozilla.firefox",
            "com.apple.Safari",
            "com.brave.Browser",
            "com.vivaldi.Vivaldi",
            "com.operasoftware.Opera",
            "company.thebrowser.Browser",
            "ai.perplexity.comet",
            "at.studio.AsideBrowser",
            "company.thebrowser.dia",
            "com.sigmaos.sigmaos.macos",
            "net.imput.helium",
            "com.nousresearch.hermes",
        ] {
            assert!(
                is_browser_bundle(bundle_id),
                "expected {bundle_id} to be treated as a browser"
            );
            assert_eq!(
                classify_surface(bundle_id, &MeetingPlatform::Zoom),
                MeetingSurface::Web
            );
        }
    }
}
