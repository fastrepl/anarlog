#![cfg(target_os = "macos")]

use hypr_activity_capture_interface::{
    ActivityCapture, AppIdKind, AppIdentity, Capabilities, CaptureAccess, CaptureError,
    CapturePolicy, CaptureStream, Snapshot, SnapshotSource, SnapshotSpec, TextAnchor, WatchOptions,
};
use objc2::rc::autoreleasepool;
use objc2_application_services::{AXIsProcessTrusted, AXUIElement};

use crate::{
    app_profile::AppProfile,
    ax::{
        TextAnchorCapture, bool_attribute, collect_generic_visible_text, collect_text_anchor,
        copy_element_attribute, enable_manual_accessibility, string_attribute,
    },
    frontmost,
    handlers::{CaptureContext, CaptureTextMode, resolve_capture_plan},
    runtime::spawn_watch_stream,
    sanitize::sanitize_snapshot_fields,
};

#[derive(Debug, Clone, Default)]
pub struct MacosCapture {
    policy: CapturePolicy,
}

impl MacosCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(policy: CapturePolicy) -> Self {
        Self { policy }
    }

    pub(crate) fn capture_snapshot(&self) -> Result<Option<Snapshot>, CaptureError> {
        autoreleasepool(|_| {
            let Some(application) = frontmost::resolve() else {
                return Ok(None);
            };
            let pid = application.pid;
            let app_name = application.app_name;
            let bundle_id = application.bundle_id;
            let app = AppIdentity {
                pid,
                app_name: app_name.clone(),
                app_id: bundle_id.clone().unwrap_or_else(|| format!("pid:{pid}")),
                app_id_kind: if bundle_id.is_some() {
                    AppIdKind::BundleId
                } else {
                    AppIdKind::Pid
                },
                bundle_id: bundle_id.clone(),
                executable_path: None,
            };
            let app_profile = AppProfile::from_bundle_id(bundle_id.as_deref());
            let app_access = self.policy.access_for_app(&app);
            if !app_access.allows_snapshot() {
                return Ok(None);
            }
            if app_access == CaptureAccess::Metadata {
                return Ok(Some(Snapshot::from_spec(SnapshotSpec {
                    captured_at: std::time::SystemTime::now(),
                    app,
                    activity_kind: hypr_activity_capture_interface::ActivityKind::ForegroundWindow,
                    access: app_access,
                    source: SnapshotSource::Workspace,
                    window_title: None,
                    url: None,
                    visible_text: None,
                    text_anchor: None,
                })));
            }

            ensure_trusted()?;

            let ax_application = unsafe { AXUIElement::new_application(pid) };
            if app_profile.prefers_manual_accessibility() {
                enable_manual_accessibility(&ax_application);
            }
            let focused_window = copy_element_attribute(&ax_application, "AXFocusedWindow")
                .or_else(|_| copy_element_attribute(&ax_application, "AXMainWindow"))?;

            let Some(focused_window) = focused_window else {
                return Ok(Some(Snapshot::from_spec(SnapshotSpec {
                    captured_at: std::time::SystemTime::now(),
                    app,
                    activity_kind: hypr_activity_capture_interface::ActivityKind::ForegroundWindow,
                    access: app_access,
                    source: SnapshotSource::Workspace,
                    window_title: None,
                    url: None,
                    visible_text: None,
                    text_anchor: None,
                })));
            };

            if bool_attribute(&focused_window, "AXMinimized")? == Some(true) {
                return Ok(None);
            }

            let bundle_id = bundle_id.filter(|value| !value.is_empty());
            let default_window_title = bundle_id.clone().unwrap_or_else(|| app_name.clone());
            let window_title = string_attribute(&focused_window, "AXTitle")?
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| default_window_title.clone());

            let plan = resolve_capture_plan(&CaptureContext {
                app: &app,
                app_profile,
                focused_window: &focused_window,
            })?;
            let decision = self.policy.decision_for_candidate(&plan.candidate);
            if decision.skip || !decision.access.allows_snapshot() {
                return Ok(None);
            }
            if !decision.access.allows_text() {
                return Ok(Some(build_snapshot(
                    app,
                    decision,
                    Some(window_title),
                    None,
                    None,
                    None,
                )));
            }

            let text_anchor =
                collect_text_anchor(&ax_application, &focused_window, &app_name, &window_title)?;
            let visible_text = match plan.text_mode {
                CaptureTextMode::Generic => {
                    collect_generic_visible_text(&ax_application, &focused_window)?
                }
                CaptureTextMode::Slack => {
                    let visible_text = crate::slack::collect_visible_text(&focused_window)?;
                    if visible_text.is_empty() {
                        collect_generic_visible_text(&ax_application, &focused_window)?
                    } else {
                        visible_text
                    }
                }
                CaptureTextMode::Spotify => {
                    let visible_text = crate::spotify::collect_visible_text()?;
                    if visible_text.is_empty() {
                        collect_generic_visible_text(&ax_application, &focused_window)?
                    } else {
                        visible_text
                    }
                }
            };

            Ok(Some(build_snapshot(
                app,
                decision,
                Some(window_title),
                Some(visible_text),
                text_anchor,
                None,
            )))
        })
    }
}

impl ActivityCapture for MacosCapture {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            can_watch: true,
            can_capture_visible_text: true,
            can_capture_browser_url: true,
            requires_accessibility_permission: true,
        }
    }

    fn snapshot(&self) -> Result<Option<Snapshot>, CaptureError> {
        self.capture_snapshot()
    }

    fn watch(&self, options: WatchOptions) -> Result<CaptureStream, CaptureError> {
        spawn_watch_stream(self.clone(), options)
    }
}

fn ensure_trusted() -> Result<(), CaptureError> {
    if unsafe { AXIsProcessTrusted() } {
        Ok(())
    } else {
        Err(CaptureError::permission_denied(
            "Accessibility permission is required for activity capture",
        ))
    }
}

fn build_snapshot(
    app: AppIdentity,
    decision: hypr_activity_capture_interface::CaptureDecision,
    window_title: Option<String>,
    visible_text: Option<String>,
    text_anchor: Option<TextAnchorCapture>,
    url_override: Option<String>,
) -> Snapshot {
    let fields = sanitize_snapshot_fields(
        &app.app_name,
        app.bundle_id.as_deref(),
        window_title,
        visible_text,
        text_anchor,
    );

    Snapshot::from_spec(SnapshotSpec {
        captured_at: std::time::SystemTime::now(),
        activity_kind: decision.activity_kind,
        access: decision.access,
        source: decision.source,
        url: url_override.or(decision.url),
        app,
        window_title: fields.window_title.filter(|value| !value.is_empty()),
        visible_text: fields.visible_text.filter(|value| !value.is_empty()),
        text_anchor: fields.text_anchor.map(|anchor| TextAnchor {
            kind: anchor.kind,
            identity: anchor.identity,
            text: anchor.text,
            prefix: anchor.prefix,
            suffix: anchor.suffix,
            selected_text: anchor.selected_text,
            confidence: anchor.confidence,
        }),
    })
}
