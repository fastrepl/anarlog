use std::future::Future;
use std::hash::{Hash, Hasher};

use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
use atspi::proxy::action::ActionProxy;
use atspi::proxy::component::ComponentProxy;
use atspi::proxy::editable_text::EditableTextProxy;
use atspi::proxy::text::TextProxy;
use atspi::{AccessibilityConnection, CoordType, ObjectRef, Role, State};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;

use super::analysis::{
    extract_chat_messages, find_participant_streams, meeting_chat_surface_is_visible,
};
use super::context::{
    browser_capture_context_id, is_platform_chat_composer, is_platform_send_button,
    native_capture_context_id, slack_capture_context_id, validated_chat_scope,
    zoom_capture_context_id,
};
use super::node::{node_labels, searchable_node_text};
use super::platform::{
    MEETING_APP_BUNDLES, classify_bundle, classify_platform, classify_surface, is_browser_bundle,
    meeting_app_family, running_apps_for_bundle, running_meeting_apps, select_active_bundle_ids,
    supports_meeting_chat_mutation, unique_recognized_meeting_bundle,
};
use super::types::{
    AxAncestor, AxNode, AxRect, BrowserMeetingRoot, MeetingApp, MeetingChatCaptureResult,
    MeetingChatSendResult, MeetingPlatform, MeetingSurface, NativeMeetingRoot, UniqueMatch,
};
use super::{
    MAX_NODES, MAX_TREE_DEPTH, MeetingAccessibilityInspection, browser_window_has_provider_signal,
    chat_input_is_owned, inspection_label, is_slack_huddle_composer_in_thread,
    is_slack_send_now_in_thread, is_slack_thread_control, native_meeting_window_is_validated,
    slack_huddle_context, slack_thread_container_path, unique_scope_for_count,
    validate_meeting_chat_message,
};

struct LiveNode {
    node: AxNode,
    ancestors: Vec<AxAncestor>,
    bus_name: String,
    path: String,
}

fn block_on_atspi<T>(future: impl Future<Output = T> + Send) -> T
where
    T: Send,
{
    // These sync entrypoints are invoked from async Tauri commands that already
    // run on the desktop Tokio runtime. Handle::block_on / Runtime::block_on on
    // that thread panics with "Cannot start a runtime from within a runtime".
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("linux AT-SPI runtime")
                    .block_on(future)
            })
            .join()
            .expect("linux AT-SPI thread panicked")
    })
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

    let text_value = if let Some(text) = TextProxy::builder(accessible.inner().connection())
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
    let value = text_value.filter(|value| !value.is_empty()).or_else(|| {
        attributes
            .get("placeholder-text")
            .cloned()
            .filter(|value| !value.is_empty())
    });
    let placeholder = attributes
        .get("placeholder-text")
        .cloned()
        .filter(|value| !value.is_empty());

    let bounds = match ComponentProxy::builder(accessible.inner().connection())
        .destination(bus_name.clone())
        .ok()?
        .path(object_path.clone())
        .ok()?
        .build()
        .await
    {
        Ok(component) => {
            component
                .get_extents(CoordType::Screen)
                .await
                .ok()
                .map(|(x, y, width, height)| AxRect {
                    x: f64::from(x),
                    y: f64::from(y),
                    width: f64::from(width),
                    height: f64::from(height),
                })
        }
        Err(_) => None,
    };

    if matches!(role.as_deref(), Some("AXStaticText")) && editable {
        // Chromium often exposes editable composers as Role::Text.
    }
    let role = if editable && matches!(role.as_deref(), Some("AXStaticText") | Some("AXGroup")) {
        Some("AXTextArea".to_string())
    } else {
        role
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
        ancestor
            .labels
            .iter()
            .any(|label| label.to_ascii_lowercase().contains("zoom meeting"))
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
    for (child_index, child) in children.into_iter().enumerate() {
        if nodes.len() >= MAX_NODES {
            *truncated = true;
            break;
        }
        let Some(child_proxy) = proxy_from_ref(connection, child).await else {
            continue;
        };
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
    windows: Vec<(Option<String>, Vec<LiveNode>)>,
    platform: &MeetingPlatform,
) -> Vec<(NativeMeetingRoot, Vec<LiveNode>)> {
    windows
        .into_iter()
        .filter_map(|(window_title, live)| {
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
    windows: Vec<(Option<String>, Vec<LiveNode>)>,
) -> Vec<(String, String, Vec<LiveNode>)> {
    windows
        .into_iter()
        .filter_map(|(_, live)| {
            let nodes = ax_nodes(&live);
            let (label, channel) = slack_huddle_context(&nodes)?;
            Some((channel, label, live))
        })
        .collect()
}

fn browser_roots_from_windows(
    windows: Vec<(Option<String>, Vec<LiveNode>)>,
    warnings: &mut Vec<String>,
) -> (Vec<(BrowserMeetingRoot, Vec<LiveNode>)>, bool) {
    let mut roots = Vec::new();
    let mut poisoned = false;
    for (window_title, live) in windows {
        let nodes = ax_nodes(&live);
        let url = web_area_url(&live);
        if !nodes
            .iter()
            .any(|node| node.role.as_deref() == Some("AXWebArea"))
        {
            if browser_window_has_provider_signal(url.as_deref(), window_title.as_deref()) {
                poisoned = true;
                warnings.push(
                    "a meeting-like browser window had no AT-SPI web area; browser capture was excluded"
                        .to_string(),
                );
            }
            continue;
        }
        let platform = super::platform::classify_browser_context(
            url.as_deref(),
            window_title.as_deref(),
            nodes
                .iter()
                .find(|node| node.role.as_deref() == Some("AXWebArea")),
            &nodes,
        );
        if platform == MeetingPlatform::Unknown {
            continue;
        }
        roots.push((
            BrowserMeetingRoot {
                platform,
                window_title,
                web_area_url: url,
                nodes,
            },
            live,
        ));
    }
    (roots, poisoned)
}

async fn collect_window_snapshots(
    connection: &AccessibilityConnection,
    app: &AccessibleProxy<'_>,
    warnings: &mut Vec<String>,
) -> Vec<(Option<String>, Vec<LiveNode>)> {
    let mut snapshots = Vec::new();
    for window in application_windows(connection, app).await {
        let title = window.name().await.ok().filter(|name| !name.is_empty());
        let (nodes, complete) = collect_app_nodes(connection, &window, warnings).await;
        if complete {
            snapshots.push((title, nodes));
        }
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
    let participant_streams = find_participant_streams(&platform, &surface, &nodes);
    let mut active_speaker_names = std::collections::HashSet::new();
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

async fn press(
    connection: &AccessibilityConnection,
    bus_name: &str,
    path: &str,
) -> Result<(), String> {
    let action = ActionProxy::builder(connection.connection())
        .destination(bus_name.to_string())
        .map_err(|error| error.to_string())?
        .path(path.to_string())
        .map_err(|error| error.to_string())?
        .build()
        .await
        .map_err(|error| error.to_string())?;
    action
        .do_action(0)
        .await
        .map_err(|error| error.to_string())
        .and_then(|ok| {
            if ok {
                Ok(())
            } else {
                Err("AT-SPI do_action returned false".to_string())
            }
        })
}

async fn text_value(
    connection: &AccessibilityConnection,
    bus_name: &str,
    path: &str,
) -> Result<String, String> {
    let text = TextProxy::builder(connection.connection())
        .destination(bus_name.to_string())
        .map_err(|error| error.to_string())?
        .path(path.to_string())
        .map_err(|error| error.to_string())?
        .build()
        .await
        .map_err(|error| error.to_string())?;
    text.get_text(0, -1)
        .await
        .map_err(|error| error.to_string())
}

async fn set_text(
    connection: &AccessibilityConnection,
    bus_name: &str,
    path: &str,
    value: &str,
) -> Result<(), String> {
    let editable = EditableTextProxy::builder(connection.connection())
        .destination(bus_name.to_string())
        .map_err(|error| error.to_string())?
        .path(path.to_string())
        .map_err(|error| error.to_string())?
        .build()
        .await
        .map_err(|error| error.to_string())?;
    editable
        .set_text_contents(value)
        .await
        .map_err(|error| error.to_string())
        .and_then(|ok| {
            if ok {
                Ok(())
            } else {
                Err("AT-SPI set_text_contents returned false".to_string())
            }
        })
}

fn slack_failure(
    app: &MeetingApp,
    surface: &MeetingSurface,
    input_label: Option<String>,
    warnings: Vec<String>,
) -> MeetingChatSendResult {
    chat_send_failure(app, &MeetingPlatform::Slack, surface, input_label, warnings)
}

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

async fn send_slack_from_live(
    connection: &AccessibilityConnection,
    app: MeetingApp,
    surface: MeetingSurface,
    channel: String,
    live: &[LiveNode],
    message: &str,
    mut warnings: Vec<String>,
) -> MeetingChatSendResult {
    let mut composers: Vec<&LiveNode> = live
        .iter()
        .filter(|node| is_slack_huddle_composer_in_thread(&node.node, &node.ancestors, &channel))
        .collect();
    if composers.is_empty() {
        let controls: Vec<&LiveNode> = live
            .iter()
            .filter(|node| is_slack_thread_control(&node.node))
            .collect();
        if let Some(control) = (controls.len() == 1).then_some(controls[0]) {
            if press(connection, &control.bus_name, &control.path)
                .await
                .is_err()
            {
                warnings.push("failed to open Slack Huddle thread via AT-SPI".to_string());
                return slack_failure(&app, &surface, None, warnings);
            }
        }
        return slack_failure(&app, &surface, None, {
            warnings.push(
                "validated Slack Huddle did not expose its composer or thread control".to_string(),
            );
            warnings
        });
    }
    if composers.len() != 1 {
        warnings.push(format!(
            "Slack Huddle exposed multiple composers for {channel}; refusing to choose one"
        ));
        return slack_failure(&app, &surface, None, warnings);
    }
    let composer = composers.remove(0);
    let Some(thread_path) =
        slack_thread_container_path(&composer.ancestors, &channel).map(<[usize]>::to_vec)
    else {
        warnings.push("Slack Huddle composer lost its thread container before send".to_string());
        return slack_failure(&app, &surface, None, warnings);
    };
    send_from_composer(
        connection,
        app,
        MeetingPlatform::Slack,
        surface,
        composer,
        live.iter()
            .filter(|node| {
                is_slack_send_now_in_thread(&node.node, &node.ancestors, &channel, &thread_path)
            })
            .collect(),
        message,
        warnings,
    )
    .await
}

async fn send_scoped_from_live(
    connection: &AccessibilityConnection,
    app: MeetingApp,
    platform: MeetingPlatform,
    surface: MeetingSurface,
    live: &[LiveNode],
    message: &str,
    mut warnings: Vec<String>,
) -> MeetingChatSendResult {
    let nodes = ax_nodes(live);
    let Some((scope_path, _)) = validated_chat_scope(&platform, &nodes) else {
        warnings.push(
            "no uniquely validated meeting chat composer is visible after inspecting the meeting window"
                .to_string(),
        );
        return chat_send_failure(&app, &platform, &surface, None, warnings);
    };
    let composers = live
        .iter()
        .filter(|node| is_platform_chat_composer(&platform, &node.node))
        .collect::<Vec<_>>();
    if composers.len() != 1 {
        warnings.push(if composers.is_empty() {
            "the meeting chat surface did not expose its composer".to_string()
        } else {
            "the meeting chat surface exposed multiple composers; refusing to choose one"
                .to_string()
        });
        return chat_send_failure(&app, &platform, &surface, None, warnings);
    }
    let send_buttons = live
        .iter()
        .filter(|node| is_platform_send_button(&platform, &node.node, &scope_path))
        .collect();
    send_from_composer(
        connection,
        app,
        platform,
        surface,
        composers[0],
        send_buttons,
        message,
        warnings,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn send_from_composer(
    connection: &AccessibilityConnection,
    app: MeetingApp,
    platform: MeetingPlatform,
    surface: MeetingSurface,
    composer: &LiveNode,
    send_buttons: Vec<&LiveNode>,
    message: &str,
    mut warnings: Vec<String>,
) -> MeetingChatSendResult {
    let input_label = inspection_label(&composer.node);
    match text_value(connection, &composer.bus_name, &composer.path).await {
        Ok(value) if value.trim().is_empty() => {}
        Ok(_) => {
            warnings.push("refusing to overwrite an existing meeting chat draft".to_string());
            return chat_send_failure(&app, &platform, &surface, input_label, warnings);
        }
        Err(error) => {
            warnings.push(format!(
                "could not verify that the meeting chat composer was empty: {error}"
            ));
            return chat_send_failure(&app, &platform, &surface, input_label, warnings);
        }
    }

    if let Err(error) = set_text(connection, &composer.bus_name, &composer.path, message).await {
        warnings.push(format!(
            "failed to set meeting chat composer value: {error}"
        ));
        return chat_send_failure(&app, &platform, &surface, input_label, warnings);
    }

    if send_buttons.len() != 1 {
        let _ = set_text(connection, &composer.bus_name, &composer.path, "").await;
        warnings.push(
            "the meeting chat composer did not expose a unique enabled send button".to_string(),
        );
        return chat_send_failure(&app, &platform, &surface, input_label, warnings);
    }

    match text_value(connection, &composer.bus_name, &composer.path).await {
        Ok(current) if chat_input_is_owned(&current, message) => {}
        Ok(_) => {
            warnings.push(
                "the meeting chat composer changed while preparing the disclosure message; nothing was sent or cleared"
                    .to_string(),
            );
            return chat_send_failure(&app, &platform, &surface, input_label, warnings);
        }
        Err(error) => {
            warnings.push(format!(
                "could not revalidate the meeting chat composer before send: {error}"
            ));
            return chat_send_failure(&app, &platform, &surface, input_label, warnings);
        }
    }

    match press(connection, &send_buttons[0].bus_name, &send_buttons[0].path).await {
        Ok(()) => MeetingChatSendResult {
            sent: true,
            app: Some(app),
            platform,
            surface,
            input_label,
            send_action: Some("sendButton".to_string()),
            warnings,
        },
        Err(error) => {
            let _ = set_text(connection, &composer.bus_name, &composer.path, "").await;
            warnings.push(format!(
                "failed to press the meeting chat send button: {error}"
            ));
            chat_send_failure(&app, &platform, &surface, input_label, warnings)
        }
    }
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
    let scoped_platform = classify_bundle(&scoped_bundle_id);
    let scoped_surface = classify_surface(&scoped_bundle_id, &scoped_platform);
    if !supports_meeting_chat_mutation(&scoped_bundle_id) {
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

    block_on_atspi(async move {
        let Ok(connection) = AccessibilityConnection::new().await else {
            return MeetingChatSendResult {
                sent: false,
                app: None,
                platform: MeetingPlatform::Unknown,
                surface: MeetingSurface::Unknown,
                input_label: None,
                send_action: None,
                warnings: vec!["AT-SPI accessibility bus is not available".to_string()],
            };
        };

        enum SendCandidate {
            SlackHuddle {
                app: MeetingApp,
                channel: String,
                live: Vec<LiveNode>,
            },
            Scoped {
                app: MeetingApp,
                platform: MeetingPlatform,
                surface: MeetingSurface,
                live: Vec<LiveNode>,
            },
        }

        let mut candidates = Vec::new();
        let mut warnings = Vec::new();
        for (app, pid) in running_apps_for_bundle(&scoped_bundle_id) {
            let Some(ax_app) = find_application_for_pid(&connection, pid, &app.name).await else {
                continue;
            };
            let windows = collect_window_snapshots(&connection, &ax_app, &mut warnings).await;
            if is_browser_bundle(&scoped_bundle_id) {
                let (roots, poisoned) = browser_roots_from_windows(windows, &mut warnings);
                if poisoned || roots.len() > 1 {
                    warnings.push(format!(
                        "refusing to send because the browser exposed {} meeting chat surfaces",
                        roots.len()
                    ));
                    return chat_send_failure(
                        &app,
                        &scoped_platform,
                        &MeetingSurface::Web,
                        None,
                        warnings,
                    );
                }
                if let Some((root, live)) = roots.into_iter().next() {
                    candidates.push(SendCandidate::Scoped {
                        app,
                        platform: root.platform,
                        surface: MeetingSurface::Web,
                        live,
                    });
                }
                continue;
            }

            if scoped_platform == MeetingPlatform::Slack {
                let mut roots = slack_roots_from_windows(windows);
                if roots.len() > 1 {
                    warnings.push(format!(
                        "refusing to send because Slack exposed {} active Huddle windows",
                        roots.len()
                    ));
                    return slack_failure(&app, &scoped_surface, None, warnings);
                }
                if let Some((channel, _, live)) = roots.pop() {
                    candidates.push(SendCandidate::SlackHuddle { app, channel, live });
                }
                continue;
            }

            let mut roots = native_roots_from_windows(windows, &scoped_platform);
            if roots.len() > 1 {
                warnings.push(format!(
                    "refusing to send because the meeting app exposed {} meeting windows",
                    roots.len()
                ));
                return chat_send_failure(&app, &scoped_platform, &scoped_surface, None, warnings);
            }
            if let Some((_, live)) = roots.pop() {
                candidates.push(SendCandidate::Scoped {
                    app,
                    platform: scoped_platform.clone(),
                    surface: scoped_surface.clone(),
                    live,
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
            Some(SendCandidate::SlackHuddle { app, channel, live }) => {
                send_slack_from_live(
                    &connection,
                    app,
                    scoped_surface,
                    channel,
                    &live,
                    &message,
                    warnings,
                )
                .await
            }
            Some(SendCandidate::Scoped {
                app,
                platform,
                surface,
                live,
            }) => {
                send_scoped_from_live(&connection, app, platform, surface, &live, &message, warnings)
                    .await
            }
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
    })
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
                        for (channel, label, live) in slack_roots_from_windows(windows) {
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
