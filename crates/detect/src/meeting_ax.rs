#[cfg(target_os = "macos")]
use cidre::{arc, ax, cf, cg};

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C-unwind" {
    #[link_name = "AXUIElementCopyAttributeValue"]
    fn ax_ui_element_copy_attribute_value_raw(
        element: &ax::UiElement,
        attribute: &ax::Attr,
        value: *mut Option<arc::R<cf::Type>>,
    ) -> i32;
    #[link_name = "AXUIElementIsAttributeSettable"]
    fn ax_ui_element_is_attribute_settable_raw(
        element: &ax::UiElement,
        attribute: &ax::Attr,
        settable: *mut bool,
    ) -> i32;
    #[link_name = "AXUIElementPerformAction"]
    fn ax_ui_element_perform_action_raw(element: &ax::UiElement, action: &ax::Action) -> i32;
    #[link_name = "AXUIElementSetAttributeValue"]
    fn ax_ui_element_set_attribute_value_raw(
        element: &ax::UiElement,
        attribute: &ax::Attr,
        value: &cf::Type,
    ) -> i32;
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
mod analysis;
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
mod context;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
mod node;
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
mod platform;
mod types;

#[cfg_attr(target_os = "linux", allow(unused_imports))]
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
use analysis::{
    candidate_chat_target, extract_chat_messages, is_slack_huddle_scope_node,
    is_zoom_chat_scope_node, is_zoom_meeting_evidence, is_zoom_meeting_scope_node,
};
#[cfg(test)]
use analysis::{
    extract_links, looks_like_time, meeting_chat_direction, meeting_chat_surface_is_visible,
    parse_chat_message, slack_huddle_is_active,
};
#[cfg(target_os = "macos")]
use context::zoom_chat_surface_is_visible;
#[cfg_attr(target_os = "linux", allow(unused_imports))]
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
use context::{
    browser_capture_context_id, is_open_meeting_chat_control, is_platform_chat_composer,
    is_platform_send_button, native_capture_context_id, path_is_ancestor, slack_capture_context_id,
    validated_chat_capture_scope, validated_chat_scope, zoom_capture_context_id,
};
#[cfg(test)]
use node::node_text;
#[cfg_attr(target_os = "linux", allow(unused_imports))]
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
use node::{
    is_platform_active_call_control, is_platform_meeting_control, node_has_positive_bounds,
    node_labels, searchable_node_text, teams_has_active_call_evidence,
};
#[cfg(any(test, target_os = "linux"))]
use platform::is_browser_active_call_control;
#[cfg_attr(target_os = "linux", allow(unused_imports))]
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
use platform::{
    MEETING_APP_BUNDLES, browser_platform_from_url, browser_title_platform_signals,
    classify_browser_context, classify_bundle, classify_platform, classify_surface,
    is_browser_bundle, select_active_bundle_ids, supports_meeting_chat_mutation,
    unique_recognized_meeting_bundle,
};
#[cfg(test)]
use platform::{MeetingAppBundleKind, is_meeting_app_bundle};
#[cfg_attr(target_os = "linux", allow(unused_imports))]
#[cfg(any(target_os = "macos", target_os = "linux"))]
use platform::{running_apps_for_bundle, running_meeting_apps};
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
use types::{
    AxAncestor, AxNode, BrowserMeetingRoot, MeetingChatTarget, NativeMeetingRoot, UniqueMatch,
};
#[cfg(target_os = "macos")]
use types::{AxChatElement, SlackHuddleRoot};
pub use types::{
    AxRect, MeetingAccessibilityInspection, MeetingApp, MeetingCapturedChatMessage,
    MeetingChatCaptureResult, MeetingChatDirection, MeetingChatSendResult, MeetingPlatform,
    MeetingSurface,
};

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
const MAX_TREE_DEPTH: usize = 32;
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
const MAX_NODES: usize = 4000;
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
const MAX_MEETING_CHAT_MESSAGE_CHARS: usize = 2_000;

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn unique_scope_for_count(count: usize) -> UniqueMatch {
    match count {
        0 => UniqueMatch::Missing,
        1 => UniqueMatch::One(0),
        _ => UniqueMatch::Ambiguous,
    }
}

#[cfg(any(test, target_os = "macos"))]
fn unique_scope_for_search(count: usize, complete: bool) -> UniqueMatch {
    if complete {
        unique_scope_for_count(count)
    } else {
        UniqueMatch::Ambiguous
    }
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
#[derive(Debug)]
enum BrowserMeetingSnapshot {
    Accept(BrowserMeetingRoot),
    Exclude,
    Unscoped,
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn is_chat_priority_label(label: &str) -> bool {
    let label = label.trim().to_ascii_lowercase();
    label.contains("in-call messages")
        || label.contains("meeting chat")
        || label.contains("send a message")
        || label.contains("type a message")
        || label.contains("type a new message")
        || label.contains("type message here")
        || label.contains("message everyone")
        || label.contains("write a message")
        || label.contains("chat message list")
        || label.contains("chat with everyone")
        || label.contains("huddle chat")
        || label.contains("open chat")
        || label.contains("open the chat panel")
        || label.contains("close the chat panel")
        || label.contains("show chat")
        || label.contains("show/hide thread")
        || label == "chat"
        || label == "leave call"
        || label == "hang up"
        || label == "leave meeting"
        || label == "end meeting"
        || label == "leave huddle"
        || label == "end huddle"
}

#[cfg(any(test, target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildWalk {
    Visible,
    Children,
}

#[cfg(any(test, target_os = "macos"))]
fn select_child_walk(
    children: Option<usize>,
    visible: Option<usize>,
    allow_visible_subset: bool,
) -> Option<ChildWalk> {
    let nonempty = |count: Option<usize>| count.filter(|&count| count > 0);
    let visible = nonempty(visible);
    let children = nonempty(children);

    match (visible, children) {
        (Some(visible_count), Some(children_count))
            if allow_visible_subset && children_count > 64 && visible_count < children_count =>
        {
            Some(ChildWalk::Visible)
        }
        (_, Some(_)) => Some(ChildWalk::Children),
        (Some(_), None) => Some(ChildWalk::Visible),
        (None, None) => None,
    }
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn browser_meeting_root_from_snapshot(
    nodes: Vec<AxNode>,
    complete: bool,
    web_area_url: Option<String>,
    window_title: Option<String>,
    web_area_node: Option<&AxNode>,
) -> BrowserMeetingSnapshot {
    let platform = classify_browser_context(
        web_area_url.as_deref(),
        window_title.as_deref(),
        web_area_node,
        &nodes,
    );
    if platform == MeetingPlatform::Unknown {
        return if !complete
            && browser_window_has_provider_signal(web_area_url.as_deref(), window_title.as_deref())
        {
            BrowserMeetingSnapshot::Unscoped
        } else {
            BrowserMeetingSnapshot::Exclude
        };
    }

    BrowserMeetingSnapshot::Accept(BrowserMeetingRoot {
        platform,
        window_title,
        web_area_url,
        nodes,
    })
}

#[cfg(target_os = "macos")]
pub fn inspect_meeting_accessibility() -> Vec<MeetingAccessibilityInspection> {
    let accessibility_trusted = macos_accessibility_client::accessibility::application_is_trusted();
    running_meeting_apps()
        .into_iter()
        .map(|(app, pid)| inspect_app(app, pid, accessibility_trusted))
        .collect()
}

#[cfg(target_os = "linux")]
pub fn inspect_meeting_accessibility() -> Vec<MeetingAccessibilityInspection> {
    linux::inspect_meeting_accessibility()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn inspect_meeting_accessibility() -> Vec<MeetingAccessibilityInspection> {
    Vec::new()
}

#[cfg(target_os = "macos")]
pub fn describe_browser_ax(pid: i32) -> Vec<String> {
    let ax_app = ax::UiElement::with_app_pid(pid);
    let _ = ax_app.set_messaging_timeout_secs(0.6);
    let mut windows = Vec::new();
    let mut visited = 0;
    let complete = collect_window_elements(&ax_app, 0, &mut visited, &mut windows);
    let focused = focused_web_area_element(&ax_app);
    let mut hits = Vec::new();
    let mut searched = 0;
    let mut ancestors = Vec::new();
    find_chat_priority_hits(&ax_app, 0, &mut searched, &mut ancestors, &mut hits);
    let mut lines = vec![format!(
        "windows={} discovery_complete={} focused_web_area={} chat_hits={} searched={}",
        windows.len(),
        complete,
        focused.is_some(),
        hits.len(),
        searched
    )];
    lines.extend(hits.into_iter().take(12));
    for (index, window) in windows.iter().enumerate() {
        let title = string_attr(window, ax::attr::title());
        let (web_area, web_complete) = active_web_area_element(window, focused.as_deref());
        let url = web_area.as_ref().and_then(|area| url_attr(area));
        let web_title = web_area
            .as_ref()
            .and_then(|area| string_attr(area, ax::attr::title()));
        let children = web_area
            .as_ref()
            .and_then(|area| ax_element_array(area, ax::attr::children()))
            .map(|array| array.len());
        let visible = web_area
            .as_ref()
            .and_then(|area| ax_element_array(area, ax::attr::visible_children()))
            .map(|array| array.len());
        lines.push(format!(
            "window[{index}] title={title:?} web_complete={web_complete} web_title={web_title:?} url={url:?} children={children:?} visible={visible:?}"
        ));
        let mut web_areas = Vec::new();
        let mut visited_areas = 0;
        let listed = collect_web_area_elements(window, 0, &mut visited_areas, &mut web_areas);
        lines.push(format!(
            "  web_areas={} list_complete={}",
            web_areas.len(),
            listed
        ));
        for (area_index, area) in web_areas.iter().enumerate() {
            lines.push(format!(
                "  web[{area_index}] title={:?} url={:?}",
                string_attr(area, ax::attr::title()),
                url_attr(area)
            ));
        }
    }
    lines
}

#[cfg(not(target_os = "macos"))]
pub fn describe_browser_ax(_pid: i32) -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn find_chat_priority_hits(
    element: &ax::UiElement,
    depth: usize,
    visited: &mut usize,
    ancestors: &mut Vec<String>,
    hits: &mut Vec<String>,
) {
    if depth > MAX_TREE_DEPTH || *visited >= MAX_NODES || hits.len() >= 12 {
        return;
    }
    *visited += 1;
    let role = string_attr(element, ax::attr::role()).unwrap_or_default();
    let role_description = string_attr(element, ax::attr::role_desc()).unwrap_or_default();
    let title = string_attr(element, ax::attr::title()).unwrap_or_default();
    let description = string_attr(element, ax::attr::desc()).unwrap_or_default();
    let help = string_attr(element, ax::attr::help()).unwrap_or_default();
    let placeholder = string_attr(element, ax::attr::placeholder_value()).unwrap_or_default();
    if is_chat_priority_label(&title)
        || is_chat_priority_label(&description)
        || is_chat_priority_label(&help)
        || is_chat_priority_label(&placeholder)
        || matches!(role.as_str(), "AXTextArea" | "AXTextField")
        || role_description.contains("text entry")
    {
        hits.push(format!(
            "  hit d{depth} role={role:?} role_desc={role_description:?} title={title:?} desc={description:?} help={help:?} placeholder={placeholder:?} settable={} bounds={} path={}",
            ax_is_settable(element, ax::attr::value()),
            ax_frame(element).is_some(),
            ancestors.join(" > ")
        ));
    }
    let Some(children) = walkable_children(element, depth == 0) else {
        return;
    };
    ancestors.push(if title.is_empty() {
        role
    } else {
        format!("{role}:{title}")
    });
    for child in children.iter() {
        find_chat_priority_hits(child, depth + 1, visited, ancestors, hits);
        if hits.len() >= 12 {
            break;
        }
    }
    ancestors.pop();
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
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

    enum SendCandidate {
        SlackHuddle {
            app: MeetingApp,
            root: SlackHuddleRoot,
        },
        Scoped {
            app: MeetingApp,
            platform: MeetingPlatform,
            surface: MeetingSurface,
            element: arc::R<ax::UiElement>,
        },
    }

    let mut candidates = Vec::new();
    let mut warnings = Vec::new();
    for (app, pid) in running_apps_for_bundle(scoped_bundle_id) {
        let ax_app = ax::UiElement::with_app_pid(pid);
        let _ = ax_app.set_messaging_timeout_secs(0.6);
        if is_browser_bundle(scoped_bundle_id) {
            let (roots, poisoned) = collect_browser_meeting_windows(&ax_app, &mut warnings);
            if poisoned || roots.len() > 1 {
                warnings.push(format!(
                    "refusing to send because the browser exposed {} meeting chat surfaces",
                    roots.len()
                ));
                return MeetingChatSendResult {
                    sent: false,
                    app: Some(app),
                    platform: scoped_platform,
                    surface: MeetingSurface::Web,
                    input_label: None,
                    send_action: None,
                    warnings,
                };
            }
            if let Some((root, element)) = roots.into_iter().next() {
                candidates.push(SendCandidate::Scoped {
                    app,
                    platform: root.platform,
                    surface: MeetingSurface::Web,
                    element,
                });
            }
            continue;
        }

        if scoped_platform == MeetingPlatform::Slack {
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
                candidates.push(SendCandidate::SlackHuddle { app, root });
            }
            continue;
        }

        let mut roots =
            collect_native_meeting_windows(&ax_app, &scoped_platform, true, &mut warnings);
        if roots.len() > 1 {
            warnings.push(format!(
                "refusing to send because the meeting app exposed {} meeting windows",
                roots.len()
            ));
            return chat_send_failure(&app, &scoped_platform, &scoped_surface, None, warnings);
        }
        if let Some((_, element)) = roots.pop() {
            candidates.push(SendCandidate::Scoped {
                app,
                platform: scoped_platform.clone(),
                surface: scoped_surface.clone(),
                element,
            });
        }
    }

    if candidates.len() > 1 {
        return MeetingChatSendResult {
            sent: false,
            app: None,
            platform: scoped_platform,
            surface: scoped_surface,
            input_label: None,
            send_action: None,
            warnings: vec![
                "refusing to send because multiple running meeting apps expose a chat composer"
                    .to_string(),
            ],
        };
    }

    match candidates.pop() {
        Some(SendCandidate::SlackHuddle { app, root }) => {
            let surface = classify_surface(&app.id, &MeetingPlatform::Slack);
            send_slack_huddle_chat_message(&app, &surface, root, &message, warnings)
        }
        Some(SendCandidate::Scoped {
            app,
            platform,
            surface,
            element,
        }) => send_scoped_chat_message(&app, &platform, &surface, &element, &message, warnings),
        None => MeetingChatSendResult {
            sent: false,
            app: None,
            platform: scoped_platform,
            surface: scoped_surface,
            input_label: None,
            send_action: None,
            warnings: vec![
                "no uniquely validated meeting chat composer is visible; AX chat mutation stays fail-closed until the window, composer, and send control can be paired"
                    .to_string(),
            ],
        },
    }
}

#[cfg(target_os = "linux")]
pub fn send_meeting_chat_message(
    message: String,
    mic_active_bundle_ids: Vec<String>,
) -> MeetingChatSendResult {
    linux::send_meeting_chat_message(message, mic_active_bundle_ids)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
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
        warnings: vec!["meeting chat AX send is only available on macOS and Linux".to_string()],
    }
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

    if let Some(children) = walkable_children(element, depth == 0) {
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
                let Some(context_id) = browser_capture_context_id(&root) else {
                    warnings.push(format!(
                        "a classified {:?} browser meeting root lacked one validated chat capture scope",
                        root.platform,
                    ));
                    return None;
                };
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

#[cfg(target_os = "linux")]
pub fn capture_meeting_chat_messages(bundle_ids: Vec<String>) -> MeetingChatCaptureResult {
    linux::capture_meeting_chat_messages(bundle_ids)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn capture_meeting_chat_messages(_bundle_ids: Vec<String>) -> MeetingChatCaptureResult {
    MeetingChatCaptureResult {
        app: None,
        platform: MeetingPlatform::Unknown,
        surface: MeetingSurface::Unknown,
        context_id: None,
        messages: Vec::new(),
        warnings: vec!["meeting chat AX capture is only available on macOS and Linux".to_string()],
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

                match ax_perform_action(&control.element, ax::action::press()) {
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
    let _ = ax_perform_action(&input_element, ax::action::press());
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
    if let Err(error) = ax_set_attr(
        &input_element,
        ax::attr::value(),
        message_value.as_type_ref(),
    ) {
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
    match ax_perform_action(&button.element, ax::action::press()) {
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
fn send_scoped_chat_message(
    app: &MeetingApp,
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
    root: &ax::UiElement,
    message: &str,
    mut warnings: Vec<String>,
) -> MeetingChatSendResult {
    let mut refreshed_nodes = Vec::new();
    if !collect_nodes(root, 0, &mut refreshed_nodes, &mut warnings) {
        warnings.push("refusing to send from an incomplete meeting AX snapshot".to_string());
        return chat_send_failure(app, platform, surface, None, warnings);
    }

    let mut chat_elements = collect_sorted_chat_elements(root);
    if validated_chat_scope(platform, &refreshed_nodes).is_none() {
        match unique_matching_chat_element_index(&chat_elements, |element| {
            is_open_meeting_chat_control(&element.node)
        }) {
            UniqueMatch::One(control_index) => {
                let control = &chat_elements[control_index];
                let label = inspection_label(&control.node)
                    .unwrap_or_else(|| "meeting chat control".to_string());
                match ax_perform_action(&control.element, ax::action::press()) {
                    Ok(_) => {
                        warnings.push(format!("opened meeting chat via AX: {label}"));
                        refreshed_nodes.clear();
                        if !collect_nodes(root, 0, &mut refreshed_nodes, &mut warnings) {
                            warnings.push(
                                "refusing to send from an incomplete meeting AX snapshot after opening chat"
                                    .to_string(),
                            );
                            return chat_send_failure(app, platform, surface, None, warnings);
                        }
                        chat_elements = collect_sorted_chat_elements(root);
                    }
                    Err(error) => {
                        warnings.push(format!("failed to open meeting chat via AX: {error:?}"));
                        return chat_send_failure(app, platform, surface, None, warnings);
                    }
                }
            }
            UniqueMatch::Missing | UniqueMatch::Ambiguous => {}
        }
    }

    let Some((scope_path, _)) = validated_chat_scope(platform, &refreshed_nodes) else {
        warnings.push(
            "no uniquely validated meeting chat composer is visible after inspecting the meeting window"
                .to_string(),
        );
        return chat_send_failure(app, platform, surface, None, warnings);
    };

    let input_index = match unique_matching_chat_element_index(&chat_elements, |element| {
        is_platform_chat_composer(platform, &element.node)
    }) {
        UniqueMatch::One(index) => index,
        UniqueMatch::Missing => {
            warnings.push("the meeting chat surface did not expose its composer".to_string());
            return chat_send_failure(app, platform, surface, None, warnings);
        }
        UniqueMatch::Ambiguous => {
            warnings.push(
                "the meeting chat surface exposed multiple composers; refusing to choose one"
                    .to_string(),
            );
            return chat_send_failure(app, platform, surface, None, warnings);
        }
    };

    let input = &chat_elements[input_index];
    let label = inspection_label(&input.node);
    let mut input_element = input.element.retained();
    let _ = ax_perform_action(&input_element, ax::action::press());
    let original_value = match chat_input_value(&input_element) {
        Ok(value) if value.trim().is_empty() => value,
        Ok(_) => {
            warnings.push("refusing to overwrite an existing meeting chat draft".to_string());
            return chat_send_failure(app, platform, surface, label, warnings);
        }
        Err(error) => {
            warnings.push(format!(
                "could not verify that the meeting chat composer was empty: {error}"
            ));
            return chat_send_failure(app, platform, surface, label, warnings);
        }
    };

    let message_value = cf::String::from_str(message);
    if let Err(error) = ax_set_attr(
        &input_element,
        ax::attr::value(),
        message_value.as_type_ref(),
    ) {
        restore_chat_input_if_owned(&mut input_element, message, &original_value, &mut warnings);
        warnings.push(format!(
            "failed to set meeting chat composer value: {error:?}"
        ));
        return chat_send_failure(app, platform, surface, label, warnings);
    }

    let (refreshed_elements, send_button_match) = collect_until_unique_match(root, |element| {
        is_platform_send_button(platform, &element.node, &scope_path)
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
                "the meeting chat composer did not expose a unique enabled send button".to_string(),
            );
            return chat_send_failure(app, platform, surface, label, warnings);
        }
        UniqueMatch::Ambiguous => {
            restore_chat_input_if_owned(
                &mut input_element,
                message,
                &original_value,
                &mut warnings,
            );
            warnings.push(
                "the meeting chat surface exposed multiple send buttons; refusing to choose one"
                    .to_string(),
            );
            return chat_send_failure(app, platform, surface, label, warnings);
        }
    };

    match chat_input_value(&input_element) {
        Ok(current) if chat_input_is_owned(&current, message) => {}
        Ok(_) => {
            warnings.push(
                "the meeting chat composer changed while preparing the disclosure message; nothing was sent or cleared"
                    .to_string(),
            );
            return chat_send_failure(app, platform, surface, label, warnings);
        }
        Err(error) => {
            warnings.push(format!(
                "could not revalidate the meeting chat composer before send: {error}"
            ));
            return chat_send_failure(app, platform, surface, label, warnings);
        }
    }

    let button = &refreshed_elements[button_index];
    match ax_perform_action(&button.element, ax::action::press()) {
        Ok(_) => MeetingChatSendResult {
            sent: true,
            app: Some(app.clone()),
            platform: platform.clone(),
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
            warnings.push(format!(
                "failed to press the meeting chat send button: {error:?}"
            ));
            chat_send_failure(app, platform, surface, label, warnings)
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
    chat_send_failure(app, &MeetingPlatform::Slack, surface, input_label, warnings)
}

#[cfg(target_os = "macos")]
fn chat_send_failure(
    app: &MeetingApp,
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
    input_label: Option<String>,
    warnings: Vec<String>,
) -> MeetingChatSendResult {
    MeetingChatSendResult {
        sent: false,
        app: Some(app.clone()),
        platform: platform.clone(),
        surface: surface.clone(),
        input_label,
        send_action: None,
        warnings,
    }
}

#[cfg(target_os = "macos")]
fn chat_input_value(input: &ax::UiElement) -> Result<String, String> {
    let value = ax_attr_value(input, ax::attr::value())
        .map_err(|status| format!("AXUIElementCopyAttributeValue failed with {status}"))?;
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
            if let Err(error) = ax_set_attr(input, ax::attr::value(), original.as_type_ref()) {
                warnings.push(format!(
                    "failed to restore the unsent meeting chat composer: {error:?}"
                ));
            }
        }
        Ok(_) => warnings.push(
            "meeting chat composer changed concurrently; its current value was left untouched"
                .to_string(),
        ),
        Err(error) => warnings.push(format!(
            "could not verify ownership of the meeting chat composer during cleanup: {error}"
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
    collect_native_meeting_windows(ax_app, platform, false, warnings)
        .into_iter()
        .map(|(root, _)| root)
        .collect()
}

#[cfg(target_os = "macos")]
fn collect_native_meeting_windows(
    ax_app: &ax::UiElement,
    platform: &MeetingPlatform,
    require_complete: bool,
    warnings: &mut Vec<String>,
) -> Vec<(NativeMeetingRoot, arc::R<ax::UiElement>)> {
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
            let complete = collect_nodes(&element, 0, &mut nodes, warnings);
            native_meeting_root_from_snapshot(
                platform,
                window_title,
                nodes,
                complete,
                require_complete,
            )
            .map(|root| (root, element))
        })
        .collect()
}

#[cfg(any(test, target_os = "macos"))]
fn native_meeting_root_from_snapshot(
    platform: &MeetingPlatform,
    window_title: Option<String>,
    nodes: Vec<AxNode>,
    complete: bool,
    require_complete: bool,
) -> Option<NativeMeetingRoot> {
    if (*platform == MeetingPlatform::Webex
        && window_title.as_deref().is_some_and(|title| {
            title
                .trim()
                .eq_ignore_ascii_case("Webex multitasking floating window")
        }))
        || (require_complete && !complete)
    {
        return None;
    }

    native_meeting_window_is_validated(platform, &nodes).then_some(NativeMeetingRoot {
        window_title,
        nodes,
    })
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn native_meeting_window_is_validated(platform: &MeetingPlatform, nodes: &[AxNode]) -> bool {
    match platform {
        MeetingPlatform::Zoom => {
            nodes.iter().any(is_zoom_meeting_evidence)
                || (nodes.iter().any(is_zoom_meeting_scope_node)
                    && nodes
                        .iter()
                        .any(|node| is_platform_active_call_control(platform, node)))
        }
        MeetingPlatform::Discord => nodes.iter().any(|node| {
            node_labels(node).any(|label| label.trim().eq_ignore_ascii_case("voice connected"))
        }),
        MeetingPlatform::MicrosoftTeams => {
            nodes
                .iter()
                .any(|node| is_platform_active_call_control(platform, node))
                || teams_has_active_call_evidence(nodes)
        }
        MeetingPlatform::Webex => nodes
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
    let (roots, poisoned) = collect_browser_meeting_windows(ax_app, warnings);
    (roots.into_iter().map(|(root, _)| root).collect(), poisoned)
}

#[cfg(target_os = "macos")]
fn collect_browser_meeting_windows(
    ax_app: &ax::UiElement,
    warnings: &mut Vec<String>,
) -> (Vec<(BrowserMeetingRoot, arc::R<ax::UiElement>)>, bool) {
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
            let complete = collect_nodes(&window, 0, &mut nodes, &mut root_warnings);
            match browser_meeting_root_from_snapshot(
                nodes,
                complete,
                web_area_url.clone(),
                window_title.clone(),
                Some(&web_area_node),
            ) {
                BrowserMeetingSnapshot::Accept(root) => {
                    warnings.extend(root_warnings);
                    Some((root, window))
                }
                BrowserMeetingSnapshot::Unscoped => {
                    has_unscoped_meeting_window = true;
                    warnings.extend(root_warnings);
                    None
                }
                BrowserMeetingSnapshot::Exclude => {
                    if browser_window_has_provider_signal(
                        web_area_url.as_deref(),
                        window_title.as_deref(),
                    ) {
                        warnings.push(
                            "a browser window lacked matching meeting-origin and title/control signals; it was excluded"
                                .to_string(),
                        );
                    }
                    None
                }
            }
        })
        .collect();

    (roots, has_unscoped_meeting_window)
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn browser_window_has_provider_signal(url: Option<&str>, title: Option<&str>) -> bool {
    browser_platform_from_url(url).is_some()
        || title.is_some_and(|title| !browser_title_platform_signals(title).is_empty())
}

#[cfg(target_os = "macos")]
fn focused_web_area_element(ax_app: &ax::UiElement) -> Option<arc::R<ax::UiElement>> {
    let mut element = ax_element_attr(ax_app, ax::attr::focused_ui_element())?;
    for _ in 0..=MAX_TREE_DEPTH {
        if string_attr(&element, ax::attr::role()).as_deref() == Some("AXWebArea") {
            return Some(element);
        }
        element = ax_element_attr(&element, ax::attr::parent())?;
    }
    None
}

#[cfg(target_os = "macos")]
fn active_web_area_element(
    window: &ax::UiElement,
    focused_web_area: Option<&ax::UiElement>,
) -> (Option<arc::R<ax::UiElement>>, bool) {
    if let Some(focused_web_area) = focused_web_area {
        let belongs_to_window = ax_element_attr(focused_web_area, ax::attr::window())
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

    let Some(role) = string_attr(element, ax::attr::role()) else {
        return false;
    };
    if role == "AXWebArea" {
        web_areas.push(element.retained());
        return true;
    }

    let Some(children) = ax_element_array(element, ax::attr::children()) else {
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

    let Some(role) = string_attr(element, ax::attr::role()) else {
        return false;
    };
    if role == "AXApplication" {
        let Some(app_windows) = ax_element_array(element, ax::attr::windows()) else {
            return false;
        };
        for window in app_windows.iter().map(ax::UiElement::retained).chain(
            [ax::attr::main_window(), ax::attr::focused_window()]
                .into_iter()
                .filter_map(|attr| ax_element_attr(element, attr)),
        ) {
            if !windows.iter().any(|existing| existing.equal(&window)) {
                windows.push(window);
            }
        }
        return true;
    }
    if role == "AXWindow" {
        windows.push(element.retained());
        return true;
    }

    let Some(children) = ax_element_array(element, ax::attr::children()) else {
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

#[cfg(test)]
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

    let Some(children) = walkable_children(element, depth == 0) else {
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

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
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
    MeetingAccessibilityInspection {
        app,
        pid,
        platform,
        surface,
        accessibility_trusted,
        window_title,
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
    maybe_note_visible_child_walk(element, warnings);
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

    let Some(children) = walkable_children(element, tree_path.is_empty()) else {
        return;
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
fn ax_element_array(
    element: &ax::UiElement,
    attr: &ax::Attr,
) -> Option<arc::R<cf::ArrayOf<ax::UiElement>>> {
    let value = ax_attr_value(element, attr).ok()?;
    if value.get_type_id() != cf::Array::type_id() {
        return None;
    }
    Some(unsafe {
        std::mem::transmute::<arc::R<cf::Type>, arc::R<cf::ArrayOf<ax::UiElement>>>(value)
    })
}

#[cfg(target_os = "macos")]
fn ax_element_attr(element: &ax::UiElement, attr: &ax::Attr) -> Option<arc::R<ax::UiElement>> {
    let value = ax_attr_value(element, attr).ok()?;
    if value.get_type_id() != ax::UiElement::type_id() {
        return None;
    }
    Some(unsafe { std::mem::transmute::<arc::R<cf::Type>, arc::R<ax::UiElement>>(value) })
}

#[cfg(target_os = "macos")]
fn walkable_children(
    element: &ax::UiElement,
    allow_visible_subset: bool,
) -> Option<arc::R<cf::ArrayOf<ax::UiElement>>> {
    let children = ax_element_array(element, ax::attr::children());
    if children
        .as_ref()
        .is_some_and(|array| !array.is_empty() && (!allow_visible_subset || array.len() <= 64))
    {
        return children;
    }

    let visible = ax_element_array(element, ax::attr::visible_children());
    match select_child_walk(
        children.as_ref().map(|array| array.len()),
        visible.as_ref().map(|array| array.len()),
        allow_visible_subset,
    ) {
        Some(ChildWalk::Visible) => visible,
        Some(ChildWalk::Children) => children,
        None => None,
    }
}

#[cfg(target_os = "macos")]
fn maybe_note_visible_child_walk(element: &ax::UiElement, warnings: &mut Vec<String>) {
    let Some(children) = ax_element_array(element, ax::attr::children()) else {
        return;
    };
    let Some(visible) = ax_element_array(element, ax::attr::visible_children()) else {
        return;
    };
    if select_child_walk(Some(children.len()), Some(visible.len()), true)
        != Some(ChildWalk::Visible)
    {
        return;
    }
    warnings.push(format!(
        "using AXVisibleChildren at the meeting root ({} visible of {} children)",
        visible.len(),
        children.len()
    ));
}

#[cfg(target_os = "macos")]
fn snapshot_node(element: &ax::UiElement, index: usize) -> AxNode {
    let element_hash = Some(element.hash());
    let role = string_attr(element, ax::attr::role());
    let identifier = string_attr(element, ax::attr::id());
    let title = string_attr(element, ax::attr::title());
    let value = string_attr(element, ax::attr::value());
    let description =
        string_attr(element, ax::attr::desc()).or_else(|| string_attr(element, ax::attr::help()));
    let placeholder = string_attr(element, ax::attr::placeholder_value());
    let enabled = ax_bool_attr(element, ax::attr::enabled());
    let settable_value = ax_is_settable(element, ax::attr::value());
    let bounds = ax_frame(element)
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
    let value = ax_attr_value(element, attr).ok()?;
    value.try_as_string().map(|s| s.to_string())
}

#[cfg(target_os = "macos")]
fn url_attr(element: &ax::UiElement) -> Option<String> {
    let value = ax_attr_value(element, ax::attr::url()).ok()?;
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
fn ax_attr_value(element: &ax::UiElement, attribute: &ax::Attr) -> Result<arc::R<cf::Type>, i32> {
    let mut value = None;
    // cidre's closed AXError enum aborts on valid error codes returned by stale UI elements.
    let status = unsafe { ax_ui_element_copy_attribute_value_raw(element, attribute, &mut value) };
    if status == 0 {
        value.ok_or(-1)
    } else {
        Err(status)
    }
}

#[cfg(target_os = "macos")]
fn ax_bool_attr(element: &ax::UiElement, attribute: &ax::Attr) -> Option<bool> {
    let value = ax_attr_value(element, attribute).ok()?;
    if value.get_type_id() != cf::Boolean::type_id() {
        return None;
    }
    let value_ref: &cf::Type = &value;
    let boolean = unsafe { &*(std::ptr::from_ref(value_ref).cast::<cf::Boolean>()) };
    Some(boolean.value())
}

#[cfg(target_os = "macos")]
fn ax_value_attr(element: &ax::UiElement, attribute: &ax::Attr) -> Option<arc::R<ax::Value>> {
    let value = ax_attr_value(element, attribute).ok()?;
    if value.get_type_id() != ax::Value::type_id() {
        return None;
    }
    Some(unsafe { std::mem::transmute::<arc::R<cf::Type>, arc::R<ax::Value>>(value) })
}

#[cfg(target_os = "macos")]
fn ax_frame(element: &ax::UiElement) -> Option<cg::Rect> {
    ax_value_attr(element, ax::attr::frame())?.cg_rect()
}

#[cfg(target_os = "macos")]
fn ax_is_settable(element: &ax::UiElement, attribute: &ax::Attr) -> bool {
    let mut settable = false;
    let status =
        unsafe { ax_ui_element_is_attribute_settable_raw(element, attribute, &mut settable) };
    status == 0 && settable
}

#[cfg(target_os = "macos")]
fn ax_perform_action(element: &ax::UiElement, action: &ax::Action) -> Result<(), i32> {
    let status = unsafe { ax_ui_element_perform_action_raw(element, action) };
    (status == 0).then_some(()).ok_or(status)
}

#[cfg(target_os = "macos")]
fn ax_set_attr(element: &ax::UiElement, attribute: &ax::Attr, value: &cf::Type) -> Result<(), i32> {
    let status = unsafe { ax_ui_element_set_attribute_value_raw(element, attribute, value) };
    (status == 0).then_some(()).ok_or(status)
}

#[cfg(target_os = "macos")]
fn rect_from_position_and_size(element: &ax::UiElement) -> Option<cg::Rect> {
    let position = ax_value_attr(element, ax::attr::pos())?.cg_point()?;
    let size = ax_value_attr(element, ax::attr::size())?.cg_size()?;
    Some(cg::Rect {
        origin: position,
        size,
    })
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn slack_huddle_context(nodes: &[AxNode]) -> Option<(String, String)> {
    let has_leave_control = nodes.iter().any(is_enabled_slack_leave_control);
    if !has_leave_control {
        return None;
    }

    if let Some(context) = nodes.iter().find_map(|node| {
        node_labels(node).find_map(|label| {
            slack_huddle_channel_from_label(label).map(|channel| (label.to_string(), channel))
        })
    }) {
        return Some(context);
    }

    None
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
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

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn is_enabled_slack_leave_control(node: &AxNode) -> bool {
    matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
        && node.enabled != Some(false)
        && node_labels(node).any(|label| label.trim().eq_ignore_ascii_case("leave huddle"))
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn is_slack_huddle_composer(node: &AxNode, channel: &str) -> bool {
    let expected = format!("message to {channel}");
    matches!(
        node.role.as_deref(),
        Some("AXTextArea") | Some("AXTextField")
    ) && node.enabled != Some(false)
        && node.settable_value
        && node_labels(node).any(|label| label.trim().eq_ignore_ascii_case(&expected))
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
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

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn is_slack_thread_container_label(label: &str, channel: &str) -> bool {
    let label = label.trim().to_ascii_lowercase();
    let expected = format!("thread in {}", channel.trim()).to_ascii_lowercase();
    label == expected || label.starts_with(&format!("{expected} ("))
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn is_slack_huddle_composer_in_thread(
    node: &AxNode,
    ancestors: &[AxAncestor],
    channel: &str,
) -> bool {
    is_slack_huddle_composer(node, channel)
        && slack_thread_container_path(ancestors, channel).is_some()
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn is_slack_send_now_in_thread(
    node: &AxNode,
    ancestors: &[AxAncestor],
    channel: &str,
    thread_path: &[usize],
) -> bool {
    is_slack_send_now_button(node)
        && slack_thread_container_path(ancestors, channel) == Some(thread_path)
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn is_slack_thread_control(node: &AxNode) -> bool {
    matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
        && node.enabled != Some(false)
        && node_labels(node).any(|label| label.trim().eq_ignore_ascii_case("show/hide thread"))
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn is_slack_send_now_button(node: &AxNode) -> bool {
    matches!(node.role.as_deref(), Some("AXButton") | Some("AXMenuItem"))
        && node.enabled != Some(false)
        && node_labels(node).any(|label| label.trim().eq_ignore_ascii_case("send now"))
}

#[cfg(test)]
fn has_nonempty_draft(node: &AxNode) -> bool {
    node.value
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
fn chat_input_is_owned(current_value: &str, injected_message: &str) -> bool {
    current_value == injected_message
}

#[cfg(test)]
mod tests;
