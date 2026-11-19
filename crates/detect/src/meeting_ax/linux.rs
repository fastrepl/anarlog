use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::{OnceLock, mpsc};

use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
use atspi::proxy::component::ComponentProxy;
use atspi::proxy::text::TextProxy;
use atspi::{AccessibilityConnection, CoordType, ObjectRef, Role, State};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;

use super::analysis::{
    extract_chat_messages, meeting_chat_surface_is_visible, slack_huddle_is_active,
};
use super::context::{
    browser_capture_context_id, native_capture_context_id, slack_capture_context_id,
    zoom_capture_context_id,
};
use super::node::{node_labels, node_needs_bounds, searchable_node_text};
use super::platform::{
    MEETING_APP_BUNDLES, classify_bundle, classify_platform, classify_surface, is_browser_bundle,
    meeting_app_family, running_apps_for_bundle, running_meeting_apps, select_active_bundle_ids,
    unique_recognized_meeting_bundle,
};
use super::types::{
    AxAncestor, AxNode, AxRect, BrowserMeetingRoot, MeetingApp, MeetingChatCaptureResult,
    MeetingChatSendResult, MeetingPlatform, MeetingSurface, NativeMeetingRoot, UniqueMatch,
};
use super::{
    BrowserMeetingSnapshot, MAX_NODES, MAX_TREE_DEPTH, MeetingAccessibilityInspection,
    browser_meeting_root_from_snapshot, browser_window_has_provider_signal, is_chat_priority_label,
    is_enabled_slack_leave_control, is_slack_huddle_composer_in_thread,
    native_meeting_window_is_validated, slack_huddle_context, slack_thread_container_path,
    unique_scope_for_count, validate_meeting_chat_message,
};

#[derive(Clone)]
struct LiveNode {
    node: AxNode,
    ancestors: Vec<AxAncestor>,
    bus_name: String,
    path: String,
}

type AtspiJob = Box<dyn FnOnce(&tokio::runtime::Runtime) + Send + 'static>;

fn atspi_worker() -> &'static mpsc::Sender<AtspiJob> {
    static WORKER: OnceLock<mpsc::Sender<AtspiJob>> = OnceLock::new();

    WORKER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<AtspiJob>();
        std::thread::Builder::new()
            .name("meeting-atspi".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("linux AT-SPI runtime");
                while let Ok(job) = receiver.recv() {
                    job(&runtime);
                }
            })
            .expect("linux AT-SPI worker");
        sender
    })
}

fn block_on_atspi<T>(future: impl Future<Output = T> + Send + 'static) -> T
where
    T: Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    atspi_worker()
        .send(Box::new(move |runtime| {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.block_on(future)));
            let _ = sender.send(result);
        }))
        .expect("linux AT-SPI worker stopped");

    match receiver.recv().expect("linux AT-SPI worker stopped") {
        Ok(output) => output,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn ax_role(role: Role) -> &'static str {
    match role {
        Role::Window | Role::Frame | Role::Dialog | Role::Alert => "AXWindow",
        Role::MenuBar => "AXMenuBar",
        Role::Menu | Role::PopupMenu => "AXMenu",
        Role::MenuItem | Role::RadioMenuItem | Role::CheckMenuItem | Role::TearoffMenuItem => {
            "AXMenuItem"
        }
        Role::Button | Role::ToggleButton | Role::RadioButton | Role::Link => "AXButton",
        Role::Entry | Role::SpinButton | Role::Editbar | Role::Autocomplete => "AXTextField",
        Role::PasswordText => "AXSecureTextField",
        Role::Text
        | Role::Paragraph
        | Role::Heading
        | Role::Label
        | Role::Static
        | Role::Caption => "AXStaticText",
        Role::DocumentWeb | Role::DocumentText | Role::DocumentFrame | Role::Page => "AXWebArea",
        Role::ScrollPane => "AXScrollArea",
        Role::List | Role::ListBox | Role::DescriptionList => "AXList",
        Role::Table | Role::TreeTable | Role::Tree => "AXTable",
        Role::TableCell | Role::ListItem | Role::TreeItem => "AXCell",
        Role::TableRow => "AXRow",
        Role::Image | Role::ImageMap | Role::Video => "AXImage",
        Role::PageTab => "AXTab",
        Role::PageTabList => "AXTabGroup",
        Role::Application => "AXApplication",
        _ => "AXGroup",
    }
}

fn element_hash(bus_name: &str, path: &str) -> usize {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bus_name.hash(&mut hasher);
    path.hash(&mut hasher);
    hasher.finish() as usize
}

fn normalized_atspi_text(value: String) -> Option<String> {
    let value = value.replace('\u{fffc}', " ");
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

async fn unix_pid_for_name(connection: &zbus::Connection, name: &str) -> Option<u32> {
    let dbus = DBusProxy::new(connection).await.ok()?;
    let bus_name = BusName::try_from(name).ok()?;
    dbus.get_connection_unix_process_id(bus_name).await.ok()
}

async fn registry_root(connection: &AccessibilityConnection) -> Option<AccessibleProxy<'_>> {
    AccessibleProxy::builder(connection.connection())
        .destination("org.a11y.atspi.Registry")
        .ok()?
        .path("/org/a11y/atspi/accessible/root")
        .ok()?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .ok()
}

async fn proxy_from_ref<'a>(
    connection: &'a AccessibilityConnection,
    object: ObjectRef,
) -> Option<AccessibleProxy<'a>> {
    object
        .into_accessible_proxy(connection.connection())
        .await
        .ok()
}

async fn snapshot_live_node(
    accessible: &AccessibleProxy<'_>,
    index: usize,
    path: &[usize],
    ancestors: &[AxAncestor],
) -> Option<LiveNode> {
    let bus_name = accessible.inner().destination().as_str().to_string();
    let object_path = accessible.inner().path().as_str().to_string();
    let role = accessible
        .get_role()
        .await
        .ok()
        .map(ax_role)
        .map(str::to_string);
    let title = accessible.name().await.ok().filter(|name| !name.is_empty());
    let description = accessible
        .description()
        .await
        .ok()
        .filter(|value| !value.is_empty());
    let attributes = accessible.get_attributes().await.unwrap_or_default();
    let identifier = attributes
        .get("id")
        .cloned()
        .or_else(|| attributes.get("class").cloned());
    let states = accessible.get_state().await.ok();
    let enabled = states.as_ref().map(|state| state.contains(State::Enabled));
    let editable = states
        .as_ref()
        .is_some_and(|state| state.contains(State::Editable));
    let settable_value = editable
        || matches!(
            role.as_deref(),
            Some("AXTextField") | Some("AXTextArea") | Some("AXSecureTextField")
        );

    let role = if editable && matches!(role.as_deref(), Some("AXStaticText") | Some("AXGroup")) {
        Some("AXTextArea".to_string())
    } else {
        role
    };
    let text_value = if settable_value {
        None
    } else if let Some(text) = TextProxy::builder(accessible.inner().connection())
        .destination(bus_name.clone())
        .ok()
        .and_then(|builder| builder.path(object_path.clone()).ok())
        .map(|builder| builder.cache_properties(zbus::proxy::CacheProperties::No))
    {
        match text.build().await {
            Ok(text) => text.get_text(0, -1).await.ok(),
            Err(_) => None,
        }
    } else {
        None
    };
    let value = text_value.and_then(normalized_atspi_text).or_else(|| {
        attributes
            .get("placeholder-text")
            .cloned()
            .filter(|value| !value.is_empty())
    });
    let placeholder = attributes
        .get("placeholder-text")
        .cloned()
        .filter(|value| !value.is_empty());

    let bounds =
        if node_needs_bounds(&role, settable_value, title.as_deref()) {
            match ComponentProxy::builder(accessible.inner().connection())
                .destination(bus_name.clone())
                .ok()?
                .path(object_path.clone())
                .ok()?
                .build()
                .await
            {
                Ok(component) => component.get_extents(CoordType::Screen).await.ok().map(
                    |(x, y, width, height)| AxRect {
                        x: f64::from(x),
                        y: f64::from(y),
                        width: f64::from(width),
                        height: f64::from(height),
                    },
                ),
                Err(_) => None,
            }
        } else {
            None
        };

    let text = searchable_node_text(
        &role,
        &title,
        &value,
        &description,
        &placeholder,
        settable_value,
    );

    Some(LiveNode {
        node: AxNode {
            index,
            tree_path: path.to_vec(),
            element_hash: Some(element_hash(&bus_name, &object_path)),
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
        },
        ancestors: ancestors.to_vec(),
        bus_name,
        path: object_path,
    })
}

async fn collect_app_nodes(
    connection: &AccessibilityConnection,
    accessible: &AccessibleProxy<'_>,
    warnings: &mut Vec<String>,
) -> (Vec<LiveNode>, bool) {
    let mut nodes = Vec::new();
    let mut ancestors = Vec::new();
    let mut path = Vec::new();
    let mut truncated = false;
    collect_nodes(
        connection,
        accessible,
        0,
        &mut path,
        &mut ancestors,
        &mut nodes,
        &mut truncated,
    )
    .await;
    if truncated {
        warnings.push(format!(
            "AT-SPI tree snapshot was incomplete at depth {MAX_TREE_DEPTH} or {MAX_NODES} nodes"
        ));
    }
    (nodes, !truncated)
}

async fn collect_nodes(
    connection: &AccessibilityConnection,
    accessible: &AccessibleProxy<'_>,
    depth: usize,
    path: &mut Vec<usize>,
    ancestors: &mut Vec<AxAncestor>,
    nodes: &mut Vec<LiveNode>,
    truncated: &mut bool,
) {
    if depth > MAX_TREE_DEPTH || nodes.len() >= MAX_NODES {
        *truncated = true;
        return;
    }

    let Some(mut live) = snapshot_live_node(accessible, nodes.len(), path, ancestors).await else {
        return;
    };
    use super::{is_slack_huddle_scope_node, is_zoom_chat_scope_node, is_zoom_meeting_scope_node};
    let within_zoom_meeting_scope = ancestors.iter().any(|ancestor| {
        ancestor.labels.iter().any(|label| {
            let label = label.to_ascii_lowercase();
            label.contains("zoom meeting") || label.trim() == "meeting"
        })
    }) || is_zoom_meeting_scope_node(&live.node);
    let within_zoom_chat_scope =
        live.node.within_zoom_chat_scope || is_zoom_chat_scope_node(&live.node);
    let within_slack_huddle_scope = is_slack_huddle_scope_node(&live.node)
        || ancestors.iter().any(|ancestor| {
            ancestor
                .labels
                .iter()
                .any(|label| label.to_ascii_lowercase().contains("huddle"))
        });
    live.node.within_zoom_meeting_scope = within_zoom_meeting_scope;
    live.node.within_zoom_chat_scope = within_zoom_chat_scope || within_zoom_meeting_scope;
    live.node.within_slack_huddle_scope = within_slack_huddle_scope;
    let labels = node_labels(&live.node)
        .map(str::to_string)
        .collect::<Vec<_>>();
    ancestors.push(AxAncestor {
        path: path.clone(),
        labels,
    });
    nodes.push(live);

    let Ok(children) = accessible.get_children().await else {
        ancestors.pop();
        return;
    };
    let mut ranked = Vec::new();
    for (child_index, child) in children.into_iter().enumerate() {
        let Some(child_proxy) = proxy_from_ref(connection, child).await else {
            continue;
        };
        let name = child_proxy.name().await.unwrap_or_default();
        ranked.push((
            if is_chat_priority_label(&name) {
                0_u8
            } else {
                1
            },
            child_index,
            child_proxy,
        ));
    }
    ranked.sort_by_key(|(rank, index, _)| (*rank, *index));
    for (_, child_index, child_proxy) in ranked {
        if nodes.len() >= MAX_NODES {
            *truncated = true;
            break;
        }
        path.push(child_index);
        Box::pin(collect_nodes(
            connection,
            &child_proxy,
            depth + 1,
            path,
            ancestors,
            nodes,
            truncated,
        ))
        .await;
        path.pop();
    }
    ancestors.pop();
}

async fn application_windows<'a>(
    connection: &'a AccessibilityConnection,
    app: &AccessibleProxy<'a>,
) -> Vec<AccessibleProxy<'a>> {
    let Ok(children) = app.get_children().await else {
        return Vec::new();
    };
    let mut windows = Vec::new();
    for child in children {
        let Some(proxy) = proxy_from_ref(connection, child).await else {
            continue;
        };
        if matches!(
            proxy.get_role().await.ok(),
            Some(Role::Window | Role::Frame | Role::Dialog)
        ) {
            windows.push(proxy);
        }
    }
    windows
}

async fn find_application_for_pid<'a>(
    connection: &'a AccessibilityConnection,
    pid: i32,
    app_name: &str,
) -> Option<AccessibleProxy<'a>> {
    let root = registry_root(connection).await?;
    let children = root.get_children().await.ok()?;
    let mut name_matches = Vec::new();
    for child in children {
        let Some(proxy) = proxy_from_ref(connection, child).await else {
            continue;
        };
        let destination = proxy.inner().destination().as_str().to_string();
        if let Some(process_id) = unix_pid_for_name(connection.connection(), &destination).await
            && process_id == pid as u32
        {
            return Some(proxy);
        }
        let name = proxy.name().await.unwrap_or_default();
        if meeting_app_family(&name) == meeting_app_family(app_name)
            || name.eq_ignore_ascii_case(app_name)
        {
            name_matches.push(proxy);
        }
    }
    (name_matches.len() == 1).then(|| name_matches.remove(0))
}

fn ax_nodes(live: &[LiveNode]) -> Vec<AxNode> {
    live.iter().map(|node| node.node.clone()).collect()
}

fn web_area_url(nodes: &[LiveNode]) -> Option<String> {
    nodes.iter().find_map(|node| {
        if node.node.role.as_deref() != Some("AXWebArea") {
            return None;
        }
        node.node
            .identifier
            .clone()
            .filter(|value| value.starts_with("http"))
            .or_else(|| {
                node.node
                    .title
                    .clone()
                    .filter(|value| value.starts_with("http"))
            })
    })
}

fn native_roots_from_windows(
    windows: Vec<(Option<String>, Vec<LiveNode>, bool)>,
    platform: &MeetingPlatform,
) -> Vec<(NativeMeetingRoot, Vec<LiveNode>)> {
    windows
        .into_iter()
        .filter_map(|(window_title, live, complete)| {
            if !complete {
                return None;
            }
            let nodes = ax_nodes(&live);
            native_meeting_window_is_validated(platform, &nodes).then_some((
                NativeMeetingRoot {
                    window_title,
                    nodes,
                },
                live,
            ))
        })
        .collect()
}

fn slack_roots_from_windows(
    windows: Vec<(Option<String>, Vec<LiveNode>, bool)>,
) -> Vec<(String, String, Vec<LiveNode>)> {
    windows
        .into_iter()
        .filter_map(|(_, live, complete)| {
            if !complete {
                return None;
            }
            let nodes = ax_nodes(&live);
            let (label, channel) = slack_huddle_context(&nodes)?;
            Some((channel, label, live))
        })
        .collect()
}

fn slack_chat_roots_from_windows(
    windows: Vec<(Option<String>, Vec<LiveNode>, bool)>,
) -> Vec<(String, String, Vec<LiveNode>)> {
    let mut contexts = windows
        .iter()
        .filter(|(_, _, complete)| *complete)
        .filter_map(|(_, live, _)| slack_huddle_context(&ax_nodes(live)))
        .collect::<Vec<_>>();
    contexts.sort();
    contexts.dedup();
    let [(label, channel)] = contexts.as_slice() else {
        return Vec::new();
    };
    let Some(active_control) = windows.iter().find_map(|(_, live, complete)| {
        if !complete
            || slack_huddle_context(&ax_nodes(live)).as_ref()
                != Some(&(label.clone(), channel.clone()))
        {
            return None;
        }
        live.iter()
            .find(|node| is_enabled_slack_leave_control(&node.node))
            .cloned()
    }) else {
        return Vec::new();
    };

    windows
        .into_iter()
        .filter_map(|(_, mut live, complete)| {
            if !complete {
                return None;
            }
            let mut composers = live.iter().filter_map(|node| {
                is_slack_huddle_composer_in_thread(&node.node, &node.ancestors, channel)
                    .then(|| slack_thread_container_path(&node.ancestors, channel))
                    .flatten()
            });
            let thread_path = composers.next()?.to_vec();
            if composers.next().is_some() {
                return None;
            }
            for node in &mut live {
                if node.node.tree_path.starts_with(&thread_path) {
                    node.node.within_slack_huddle_scope = true;
                }
            }
            if !slack_huddle_is_active(&ax_nodes(&live)) {
                live.push(active_control.clone());
            }
            Some((channel.clone(), label.clone(), live))
        })
        .collect()
}

fn browser_roots_from_windows(
    windows: Vec<(Option<String>, Vec<LiveNode>, bool)>,
    warnings: &mut Vec<String>,
) -> (Vec<(BrowserMeetingRoot, Vec<LiveNode>)>, bool) {
    let mut roots = Vec::new();
    let mut poisoned = false;
    for (window_title, live, complete) in windows {
        let nodes = ax_nodes(&live);
        let url = web_area_url(&live);
        let web_area = nodes
            .iter()
            .find(|node| node.role.as_deref() == Some("AXWebArea"))
            .cloned();
        if web_area.is_none() {
            if browser_window_has_provider_signal(url.as_deref(), window_title.as_deref()) {
                poisoned = true;
                warnings.push(
                    "a meeting-like browser window had no AT-SPI web area; browser capture was excluded"
                        .to_string(),
                );
            }
            continue;
        }
        match browser_meeting_root_from_snapshot(
            nodes,
            complete,
            url,
            window_title,
            web_area.as_ref(),
        ) {
            BrowserMeetingSnapshot::Accept(root) => roots.push((root, live)),
            BrowserMeetingSnapshot::Unscoped => poisoned = true,
            BrowserMeetingSnapshot::Exclude => {}
        }
    }
    (roots, poisoned)
}

async fn collect_window_snapshots(
    connection: &AccessibilityConnection,
    app: &AccessibleProxy<'_>,
    warnings: &mut Vec<String>,
) -> Vec<(Option<String>, Vec<LiveNode>, bool)> {
    let mut snapshots = Vec::new();
    for window in application_windows(connection, app).await {
        let title = window.name().await.ok().filter(|name| !name.is_empty());
        let (nodes, complete) = collect_app_nodes(connection, &window, warnings).await;
        snapshots.push((title, nodes, complete));
    }
    snapshots
}

fn inspection_from_nodes(
    app: MeetingApp,
    pid: i32,
    accessibility_trusted: bool,
    window_title: Option<String>,
    nodes: Vec<AxNode>,
    scoped_platform: Option<MeetingPlatform>,
    warnings: Vec<String>,
) -> MeetingAccessibilityInspection {
    let bundle_platform = classify_bundle(&app.id);
    let platform = scoped_platform.unwrap_or_else(|| {
        classify_platform(&app.id, window_title.as_deref(), &nodes, bundle_platform)
    });
    let surface = classify_surface(&app.id, &platform);
    let active_call = accessibility_trusted
        && nodes
            .iter()
            .any(|node| super::is_platform_active_call_control(&platform, node));
    MeetingAccessibilityInspection {
        active_call,
        app,
        pid,
        platform,
        surface,
        accessibility_trusted,
        window_title,
        remote_screen_share: active_call && super::nodes_show_remote_screen_share(&nodes),
        warnings,
    }
}

async fn inspect_app_async(
    connection: Option<&AccessibilityConnection>,
    app: MeetingApp,
    pid: i32,
) -> MeetingAccessibilityInspection {
    let Some(connection) = connection else {
        return inspection_from_nodes(
            app,
            pid,
            false,
            None,
            Vec::new(),
            None,
            vec!["AT-SPI accessibility bus is not available".to_string()],
        );
    };

    let mut warnings = Vec::new();
    let Some(ax_app) = find_application_for_pid(connection, pid, &app.name).await else {
        warnings.push("could not match the meeting app on the AT-SPI bus".to_string());
        return inspection_from_nodes(app, pid, true, None, Vec::new(), None, warnings);
    };

    let windows = collect_window_snapshots(connection, &ax_app, &mut warnings).await;
    let bundle_platform = classify_bundle(&app.id);
    if bundle_platform == MeetingPlatform::Slack {
        let mut roots = slack_roots_from_windows(windows);
        let (window_title, nodes, scoped) = match roots.len() {
            1 => {
                let (channel, label, live) = roots.remove(0);
                let _ = channel;
                (Some(label), ax_nodes(&live), None)
            }
            0 => {
                warnings.push(
                    "Slack is running without a uniquely validated active Huddle".to_string(),
                );
                (None, Vec::new(), None)
            }
            count => {
                warnings.push(format!(
                    "Slack exposed {count} active Huddle windows; inspection is scoped to none"
                ));
                (None, Vec::new(), None)
            }
        };
        return inspection_from_nodes(app, pid, true, window_title, nodes, scoped, warnings);
    }

    if is_browser_bundle(&app.id) {
        let (mut roots, poisoned) = browser_roots_from_windows(windows, &mut warnings);
        let root_match = if poisoned {
            UniqueMatch::Ambiguous
        } else {
            unique_scope_for_count(roots.len())
        };
        return match root_match {
            UniqueMatch::One(index) => {
                let (root, _) = roots.remove(index);
                inspection_from_nodes(
                    app,
                    pid,
                    true,
                    root.window_title.clone(),
                    root.nodes,
                    Some(root.platform),
                    warnings,
                )
            }
            UniqueMatch::Missing => {
                warnings.push(
                    "browser inspection found no uniquely scoped meeting window and web area"
                        .to_string(),
                );
                inspection_from_nodes(app, pid, true, None, Vec::new(), None, warnings)
            }
            UniqueMatch::Ambiguous => {
                warnings.push(
                    "browser meeting window scope was ambiguous; inspection is scoped to none"
                        .to_string(),
                );
                inspection_from_nodes(app, pid, true, None, Vec::new(), None, warnings)
            }
        };
    }

    if bundle_platform != MeetingPlatform::Unknown {
        let mut roots = native_roots_from_windows(windows, &bundle_platform);
        return match unique_scope_for_count(roots.len()) {
            UniqueMatch::One(index) => {
                let (root, _) = roots.remove(index);
                inspection_from_nodes(
                    app,
                    pid,
                    true,
                    root.window_title,
                    root.nodes,
                    Some(bundle_platform),
                    warnings,
                )
            }
            UniqueMatch::Missing => {
                warnings.push(
                    "native app exposed no evidence-backed meeting window; inspection is scoped to none"
                        .to_string(),
                );
                inspection_from_nodes(app, pid, true, None, Vec::new(), None, warnings)
            }
            UniqueMatch::Ambiguous => {
                warnings.push(
                    "native app exposed multiple meeting windows; inspection is scoped to none"
                        .to_string(),
                );
                inspection_from_nodes(app, pid, true, None, Vec::new(), None, warnings)
            }
        };
    }

    warnings.push("app bundle has no validated meeting inspection path".to_string());
    inspection_from_nodes(
        app,
        pid,
        true,
        None,
        Vec::new(),
        Some(MeetingPlatform::Unknown),
        warnings,
    )
}

pub(super) fn inspect_meeting_accessibility() -> Vec<MeetingAccessibilityInspection> {
    block_on_atspi(async {
        let connection = AccessibilityConnection::new().await.ok();
        let mut inspections = Vec::new();
        for (app, pid) in running_meeting_apps() {
            inspections.push(inspect_app_async(connection.as_ref(), app, pid).await);
        }
        inspections
    })
}

pub(super) fn send_meeting_chat_message(
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
        Ok(bundle_id) => bundle_id.to_string(),
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
    // AX chat mutation is macOS-only (see supports_meeting_chat_mutation), so every app is refused.
    let scoped_platform = classify_bundle(&scoped_bundle_id);
    let scoped_surface = classify_surface(&scoped_bundle_id, &scoped_platform);
    MeetingChatSendResult {
        sent: false,
        app: None,
        platform: scoped_platform,
        surface: scoped_surface,
        input_label: None,
        send_action: None,
        warnings: vec![format!(
            "AX chat mutation is disabled for the mic-active meeting app {scoped_bundle_id}"
        )],
    }
}

pub(super) fn capture_meeting_chat_messages(bundle_ids: Vec<String>) -> MeetingChatCaptureResult {
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

    let bundle_id = scoped_bundle_ids[0];
    let bundle_platform = classify_bundle(bundle_id);
    let bundle_surface = classify_surface(bundle_id, &bundle_platform);

    block_on_atspi(async move {
        let Ok(connection) = AccessibilityConnection::new().await else {
            return MeetingChatCaptureResult {
                app: None,
                platform: bundle_platform,
                surface: bundle_surface,
                context_id: None,
                messages: Vec::new(),
                warnings: vec!["AT-SPI accessibility bus is not available".to_string()],
            };
        };

        let mut warnings = Vec::new();
        let mut candidates = Vec::new();
        let mut detected_platform = bundle_platform.clone();
        let running_apps = running_apps_for_bundle(bundle_id);

        if is_browser_bundle(bundle_id) {
            let mut browser_roots = Vec::new();
            let mut poisoned = false;
            for (app, pid) in running_apps {
                let Some(ax_app) = find_application_for_pid(&connection, pid, &app.name).await
                else {
                    continue;
                };
                let windows = collect_window_snapshots(&connection, &ax_app, &mut warnings).await;
                let (roots, window_poisoned) = browser_roots_from_windows(windows, &mut warnings);
                poisoned |= window_poisoned;
                browser_roots.extend(roots.into_iter().filter_map(|(root, _)| {
                    let context_id = browser_capture_context_id(&root)?;
                    Some((app.clone(), root, context_id))
                }));
            }
            if poisoned || browser_roots.len() != 1 {
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
                let Some(ax_app) = find_application_for_pid(&connection, pid, &app.name).await
                else {
                    continue;
                };
                let windows = collect_window_snapshots(&connection, &ax_app, &mut warnings).await;
                match &bundle_platform {
                    MeetingPlatform::Zoom => {
                        for (root, _) in native_roots_from_windows(windows, &MeetingPlatform::Zoom)
                        {
                            if meeting_chat_surface_is_visible(&MeetingPlatform::Zoom, &root.nodes)
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
                        for (channel, label, live) in slack_chat_roots_from_windows(windows) {
                            let composer = live.iter().find(|node| {
                                is_slack_huddle_composer_in_thread(
                                    &node.node,
                                    &node.ancestors,
                                    &channel,
                                )
                            });
                            let Some(composer) = composer else {
                                continue;
                            };
                            let context_id = slack_capture_context_id(
                                &channel,
                                &label,
                                element_hash(&composer.bus_name, &composer.path),
                                composer.node.element_hash.unwrap_or_default(),
                            );
                            candidates.push((
                                app.clone(),
                                MeetingPlatform::Slack,
                                MeetingSurface::Native,
                                context_id,
                                ax_nodes(&live),
                            ));
                        }
                    }
                    MeetingPlatform::MicrosoftTeams | MeetingPlatform::Webex => {
                        for (root, _) in native_roots_from_windows(windows, &bundle_platform) {
                            if let Some(context_id) =
                                native_capture_context_id(&bundle_platform, &root)
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atspi_worker_is_reused() {
        let first = block_on_atspi(async { std::thread::current().id() });
        let second = block_on_atspi(async { std::thread::current().id() });

        assert_eq!(first, second);
    }

    #[test]
    fn object_replacement_text_does_not_pollute_accessible_labels() {
        assert_eq!(normalized_atspi_text("\u{fffc}".to_string()), None);
        assert_eq!(
            normalized_atspi_text("Chat Message List\u{fffc}".to_string()),
            Some("Chat Message List".to_string())
        );
    }

    fn live_node(index: usize, role: &str, title: &str, path: &[usize]) -> LiveNode {
        LiveNode {
            node: AxNode {
                index,
                tree_path: path.to_vec(),
                element_hash: Some(index),
                role: Some(role.to_string()),
                identifier: None,
                title: Some(title.to_string()),
                value: None,
                description: None,
                placeholder: None,
                enabled: Some(true),
                settable_value: false,
                bounds: Some(AxRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 48.0,
                }),
                text: title.to_ascii_lowercase(),
                within_zoom_meeting_scope: false,
                within_zoom_chat_scope: false,
                within_slack_huddle_scope: false,
            },
            ancestors: Vec::new(),
            bus_name: "org.test.Slack".to_string(),
            path: format!("/org/test/{index}"),
        }
    }

    #[test]
    fn slack_chat_scope_pairs_main_huddle_identity_with_popout_thread() {
        let main = vec![
            live_node(0, "AXGroup", "Huddle in test", &[0]),
            live_node(1, "AXButton", "Leave Huddle", &[1]),
        ];
        let mut composer = live_node(2, "AXTextArea", "Message to test", &[2, 0]);
        composer.node.settable_value = true;
        composer.ancestors.push(AxAncestor {
            path: vec![2],
            labels: vec!["Thread in test (private channel, 1 reply)".to_string()],
        });
        let message = live_node(3, "AXCell", "John Jeong: Ship it. 12:03 PM.", &[2, 1]);

        let roots = slack_chat_roots_from_windows(vec![
            (
                Some("test (Channel) - Fastrepl - Slack".to_string()),
                main,
                true,
            ),
            (
                Some("test - Fastrepl - Slack".to_string()),
                vec![composer, message],
                true,
            ),
        ]);

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].0, "test");
        assert_eq!(roots[0].1, "Huddle in test");
        assert!(roots[0].2.iter().any(|node| node.node.index == 3));
        let messages = extract_chat_messages(
            &MeetingPlatform::Slack,
            &MeetingSurface::Native,
            &ax_nodes(&roots[0].2),
        );
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender.as_deref(), Some("John Jeong"));
        assert_eq!(messages[0].timestamp.as_deref(), Some("12:03 PM"));
        assert_eq!(messages[0].text, "Ship it");
    }

    #[test]
    fn slack_chat_scope_rejects_ambiguous_huddle_identity() {
        let mut composer = live_node(4, "AXTextArea", "Message to test", &[4, 0]);
        composer.node.settable_value = true;
        composer.ancestors.push(AxAncestor {
            path: vec![4],
            labels: vec!["Thread in test".to_string()],
        });

        assert!(
            slack_chat_roots_from_windows(vec![
                (
                    Some("test".to_string()),
                    vec![
                        live_node(0, "AXGroup", "Huddle in test", &[0]),
                        live_node(1, "AXButton", "Leave Huddle", &[1]),
                    ],
                    true,
                ),
                (
                    Some("random".to_string()),
                    vec![
                        live_node(2, "AXGroup", "Huddle in random", &[0]),
                        live_node(3, "AXButton", "Leave Huddle", &[1]),
                    ],
                    true,
                ),
                (Some("thread".to_string()), vec![composer], true),
            ])
            .is_empty()
        );
    }
}
