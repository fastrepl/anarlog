use std::collections::HashSet;

use windows::Win32::Foundation::{HWND, LPARAM, RECT, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::BOOL;

use super::{
    AxNode, AxRect, BrowserMeetingRoot, MAX_NODES, MAX_TREE_DEPTH, MEETING_APP_BUNDLES,
    MeetingAccessibilityInspection, MeetingApp, MeetingChatCaptureResult, MeetingPlatform,
    MeetingSurface, NativeMeetingRoot, browser_capture_context_id, classify_browser_context,
    classify_bundle, classify_surface, extract_chat_messages, is_browser_bundle,
    is_platform_active_call_control, native_capture_context_id, node_needs_bounds,
    running_apps_for_bundle, running_meeting_apps, searchable_node_text, select_active_bundle_ids,
    validated_chat_capture_scope,
};

struct ComGuard {
    initialized: bool,
}

impl ComGuard {
    fn initialize() -> Result<Self, String> {
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result == RPC_E_CHANGED_MODE {
            return Ok(Self { initialized: false });
        }
        result
            .ok()
            .map_err(|error| format!("Windows UI Automation COM initialization failed: {error}"))?;
        Ok(Self { initialized: true })
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

struct WindowEnumState {
    process_ids: HashSet<u32>,
    windows: Vec<(HWND, u32)>,
}

unsafe extern "system" fn collect_window(hwnd: HWND, state_ptr: LPARAM) -> BOOL {
    let state = unsafe { &mut *(state_ptr.0 as *mut WindowEnumState) };
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return true.into();
    }

    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if state.process_ids.contains(&process_id) {
        state.windows.push((hwnd, process_id));
    }
    true.into()
}

fn visible_windows(process_ids: impl IntoIterator<Item = u32>) -> Result<Vec<(HWND, u32)>, String> {
    let mut state = WindowEnumState {
        process_ids: process_ids.into_iter().collect(),
        windows: Vec::new(),
    };
    unsafe {
        EnumWindows(
            Some(collect_window),
            LPARAM((&mut state as *mut WindowEnumState) as isize),
        )
    }
    .map_err(|error| format!("failed to enumerate Windows meeting windows: {error}"))?;
    state.windows.sort_by_key(|(hwnd, _)| hwnd.0 as usize);
    Ok(state.windows)
}

fn window_title(hwnd: HWND) -> Option<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return None;
    }
    let mut buffer = vec![0; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    (copied > 0).then(|| String::from_utf16_lossy(&buffer[..copied as usize]))
}

fn create_automation() -> Result<IUIAutomation, String> {
    unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
        .map_err(|error| format!("failed to create Windows UI Automation client: {error}"))
}

fn create_cache_request(automation: &IUIAutomation) -> Result<IUIAutomationCacheRequest, String> {
    let request = unsafe { automation.CreateCacheRequest() }
        .map_err(|error| format!("failed to create Windows UI Automation cache: {error}"))?;
    for property in [
        UIA_AutomationIdPropertyId,
        UIA_ControlTypePropertyId,
        UIA_HelpTextPropertyId,
        UIA_IsEnabledPropertyId,
        UIA_IsOffscreenPropertyId,
        UIA_NamePropertyId,
    ] {
        unsafe { request.AddProperty(property) }
            .map_err(|error| format!("failed to configure Windows UI Automation cache: {error}"))?;
    }
    unsafe { request.SetTreeScope(TreeScope_Subtree) }
        .map_err(|error| format!("failed to set Windows UI Automation tree scope: {error}"))?;
    unsafe { request.SetAutomationElementMode(AutomationElementMode_Full) }
        .map_err(|error| format!("failed to set Windows UI Automation element mode: {error}"))?;
    Ok(request)
}

fn nonempty(value: windows::core::Result<windows::core::BSTR>) -> Option<String> {
    value
        .ok()
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
}

#[allow(non_upper_case_globals)]
fn role_for_control_type(control_type: UIA_CONTROLTYPE_ID) -> Option<String> {
    let role = match control_type {
        UIA_ButtonControlTypeId => "AXButton",
        UIA_ComboBoxControlTypeId => "AXComboBox",
        UIA_DataGridControlTypeId | UIA_TableControlTypeId => "AXTable",
        UIA_DocumentControlTypeId => "AXWebArea",
        UIA_EditControlTypeId => "AXTextField",
        UIA_GroupControlTypeId | UIA_PaneControlTypeId | UIA_CustomControlTypeId => "AXGroup",
        UIA_HeaderControlTypeId => "AXGroup",
        UIA_HeaderItemControlTypeId | UIA_TextControlTypeId => "AXStaticText",
        UIA_HyperlinkControlTypeId => "AXLink",
        UIA_ListControlTypeId => "AXList",
        UIA_ListItemControlTypeId | UIA_TreeItemControlTypeId => "AXRow",
        UIA_MenuControlTypeId => "AXMenu",
        UIA_MenuItemControlTypeId => "AXMenuItem",
        UIA_TabControlTypeId => "AXTabGroup",
        UIA_TabItemControlTypeId => "AXRadioButton",
        UIA_TreeControlTypeId => "AXOutline",
        UIA_WindowControlTypeId => "AXWindow",
        _ => return None,
    };
    Some(role.to_string())
}

fn element_is_settable(element: &IUIAutomationElement, role: Option<&str>) -> bool {
    if !matches!(
        role,
        Some("AXTextArea") | Some("AXTextField") | Some("AXComboBox")
    ) {
        return false;
    }
    let Ok(pattern) =
        (unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) })
    else {
        return false;
    };
    unsafe { pattern.CurrentIsReadOnly() }.is_ok_and(|read_only| !read_only.as_bool())
}

fn element_bounds(element: &IUIAutomationElement) -> Option<AxRect> {
    let RECT {
        left,
        top,
        right,
        bottom,
    } = unsafe { element.CurrentBoundingRectangle() }.ok()?;
    Some(AxRect {
        x: f64::from(left),
        y: f64::from(top),
        width: f64::from(right - left),
        height: f64::from(bottom - top),
    })
}

fn element_hash(
    hwnd: HWND,
    path: &[usize],
    role: Option<&str>,
    identifier: Option<&str>,
    title: Option<&str>,
) -> usize {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    for byte in (hwnd.0 as usize).to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for part in path {
        for byte in part.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    for value in [role, identifier, title].into_iter().flatten() {
        for byte in value
            .as_bytes()
            .iter()
            .copied()
            .chain(std::iter::once(0xff))
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash as usize
}

fn collect_cached_nodes(
    element: &IUIAutomationElement,
    hwnd: HWND,
    path: &mut Vec<usize>,
    nodes: &mut Vec<AxNode>,
) -> bool {
    if path.len() > MAX_TREE_DEPTH || nodes.len() >= MAX_NODES {
        return false;
    }

    let control_type = unsafe { element.CachedControlType() }.ok();
    let role = control_type.and_then(role_for_control_type);
    let identifier = nonempty(unsafe { element.CachedAutomationId() });
    let title = nonempty(unsafe { element.CachedName() });
    let description = nonempty(unsafe { element.CachedHelpText() });
    let enabled = unsafe { element.CachedIsEnabled() }
        .ok()
        .map(|value| value.as_bool());
    let offscreen = unsafe { element.CachedIsOffscreen() }
        .ok()
        .map(|value| value.as_bool());
    let settable_value = enabled != Some(false)
        && offscreen != Some(true)
        && element_is_settable(element, role.as_deref());
    let bounds = node_needs_bounds(&role, settable_value, title.as_deref())
        .then(|| element_bounds(element))
        .flatten();
    let value = None;
    let placeholder = None;
    let text = searchable_node_text(
        &role,
        &title,
        &value,
        &description,
        &placeholder,
        settable_value,
    );
    nodes.push(AxNode {
        index: nodes.len(),
        tree_path: path.clone(),
        element_hash: Some(element_hash(
            hwnd,
            path,
            role.as_deref(),
            identifier.as_deref(),
            title.as_deref(),
        )),
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
    });

    let Ok(children) = (unsafe { element.GetCachedChildren() }) else {
        return false;
    };
    let Ok(child_count) = (unsafe { children.Length() }) else {
        return false;
    };
    for child_index in 0..child_count {
        let Ok(child) = (unsafe { children.GetElement(child_index) }) else {
            return false;
        };
        path.push(child_index as usize);
        if !collect_cached_nodes(&child, hwnd, path, nodes) {
            path.pop();
            return false;
        }
        path.pop();
    }
    true
}

fn snapshot_window(
    automation: &IUIAutomation,
    cache_request: &IUIAutomationCacheRequest,
    hwnd: HWND,
) -> Result<(Vec<AxNode>, bool), String> {
    let root = unsafe { automation.ElementFromHandleBuildCache(hwnd, cache_request) }
        .map_err(|error| format!("failed to snapshot Windows UI Automation window: {error}"))?;
    let mut nodes = Vec::new();
    let complete = collect_cached_nodes(&root, hwnd, &mut Vec::new(), &mut nodes);
    Ok((nodes, complete))
}

fn mark_native_capture_scope(platform: &MeetingPlatform, nodes: &mut [AxNode]) {
    let Some((scope_path, _)) = validated_chat_capture_scope(platform, nodes) else {
        return;
    };
    for node in nodes {
        let within_scope = node.tree_path == scope_path || node.tree_path.starts_with(&scope_path);
        if *platform == MeetingPlatform::Zoom {
            node.within_zoom_meeting_scope = true;
            node.within_zoom_chat_scope = within_scope;
        }
        if *platform == MeetingPlatform::Slack {
            node.within_slack_huddle_scope = within_scope;
        }
    }
}

struct CaptureCandidate {
    app: MeetingApp,
    platform: MeetingPlatform,
    surface: MeetingSurface,
    context_id: String,
    nodes: Vec<AxNode>,
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
    let running_apps = running_apps_for_bundle(bundle_id);
    let mut warnings = Vec::new();
    if running_apps.is_empty() {
        warnings.push("the mic-active Windows meeting app has no running process".to_string());
    }

    let _com = match ComGuard::initialize() {
        Ok(com) => com,
        Err(error) => {
            return MeetingChatCaptureResult {
                app: None,
                platform: bundle_platform,
                surface: bundle_surface,
                context_id: None,
                messages: Vec::new(),
                warnings: vec![error],
            };
        }
    };
    let automation = match create_automation() {
        Ok(automation) => automation,
        Err(error) => {
            return MeetingChatCaptureResult {
                app: None,
                platform: bundle_platform,
                surface: bundle_surface,
                context_id: None,
                messages: Vec::new(),
                warnings: vec![error],
            };
        }
    };
    let cache_request = match create_cache_request(&automation) {
        Ok(request) => request,
        Err(error) => {
            return MeetingChatCaptureResult {
                app: None,
                platform: bundle_platform,
                surface: bundle_surface,
                context_id: None,
                messages: Vec::new(),
                warnings: vec![error],
            };
        }
    };
    let windows = match visible_windows(
        running_apps
            .iter()
            .filter_map(|(_, pid)| u32::try_from(*pid).ok()),
    ) {
        Ok(windows) => windows,
        Err(error) => {
            warnings.push(error);
            Vec::new()
        }
    };
    if windows.is_empty() {
        warnings
            .push("the mic-active Windows meeting app has no visible top-level window".to_string());
    }

    let mut detected_platform = bundle_platform.clone();
    let mut candidates = Vec::new();
    for (hwnd, process_id) in windows {
        let Some((app, _)) = running_apps
            .iter()
            .find(|(_, pid)| u32::try_from(*pid) == Ok(process_id))
        else {
            continue;
        };
        let title = window_title(hwnd);
        let (mut nodes, complete) = match snapshot_window(&automation, &cache_request, hwnd) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                warnings.push(error);
                continue;
            }
        };
        if !complete {
            warnings.push(format!(
                "refusing an incomplete Windows UI Automation snapshot for window {:?}",
                title
            ));
            continue;
        }

        let platform = if is_browser_bundle(bundle_id) {
            classify_browser_context(None, title.as_deref(), None, &nodes)
        } else {
            bundle_platform.clone()
        };
        if platform == MeetingPlatform::Unknown {
            continue;
        }
        detected_platform = platform.clone();
        let surface = classify_surface(bundle_id, &platform);
        if surface == MeetingSurface::Native {
            mark_native_capture_scope(&platform, &mut nodes);
        }
        let context_id = if surface == MeetingSurface::Web {
            browser_capture_context_id(&BrowserMeetingRoot {
                platform: platform.clone(),
                window_title: title,
                web_area_url: None,
                nodes: nodes.clone(),
            })
        } else {
            native_capture_context_id(
                &platform,
                &NativeMeetingRoot {
                    window_title: title,
                    nodes: nodes.clone(),
                },
            )
        };
        let Some(context_id) = context_id else {
            continue;
        };
        candidates.push(CaptureCandidate {
            app: app.clone(),
            platform,
            surface,
            context_id,
            nodes,
        });
    }

    if candidates.len() != 1 {
        warnings.push(format!(
            "meeting chat capture requires exactly one validated visible Windows UI Automation chat surface; found {}",
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

    let candidate = candidates.pop().unwrap();
    let messages = extract_chat_messages(&candidate.platform, &candidate.surface, &candidate.nodes);
    MeetingChatCaptureResult {
        app: Some(candidate.app),
        platform: candidate.platform,
        surface: candidate.surface,
        context_id: Some(candidate.context_id),
        messages,
        warnings,
    }
}

pub(super) fn inspect_meeting_accessibility() -> Vec<MeetingAccessibilityInspection> {
    let running_apps = running_meeting_apps();
    let Ok(_com) = ComGuard::initialize() else {
        return Vec::new();
    };
    let Ok(automation) = create_automation() else {
        return Vec::new();
    };
    let Ok(cache_request) = create_cache_request(&automation) else {
        return Vec::new();
    };
    let Ok(windows) = visible_windows(
        running_apps
            .iter()
            .filter_map(|(_, pid)| u32::try_from(*pid).ok()),
    ) else {
        return Vec::new();
    };

    windows
        .into_iter()
        .filter_map(|(hwnd, process_id)| {
            let (app, pid) = running_apps
                .iter()
                .find(|(_, pid)| u32::try_from(*pid) == Ok(process_id))?;
            let title = window_title(hwnd);
            let (nodes, complete) = snapshot_window(&automation, &cache_request, hwnd).ok()?;
            let bundle_platform = classify_bundle(&app.id);
            let platform = if is_browser_bundle(&app.id) {
                classify_browser_context(None, title.as_deref(), None, &nodes)
            } else {
                bundle_platform
            };
            let surface = classify_surface(&app.id, &platform);
            let has_active_call = complete
                && platform != MeetingPlatform::Unknown
                && nodes
                    .iter()
                    .any(|node| is_platform_active_call_control(&platform, node));
            let mut warnings = (!complete)
                .then(|| "Windows UI Automation snapshot was incomplete".to_string())
                .into_iter()
                .collect::<Vec<_>>();
            if !has_active_call {
                warnings.push(
                    "Windows UI Automation found no uniquely validated active meeting controls"
                        .to_string(),
                );
            }
            Some(MeetingAccessibilityInspection {
                app: app.clone(),
                pid: *pid,
                platform,
                surface,
                accessibility_trusted: true,
                window_title: has_active_call.then_some(title).flatten(),
                warnings,
            })
        })
        .collect()
}
