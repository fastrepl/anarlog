#![cfg(target_os = "macos")]

use std::time::SystemTime;

use hypr_activity_capture_interface::{
    ActivityCapture, Capabilities, CaptureAccess, CaptureError, CapturePolicy, CaptureStream,
    ContentLevel, Snapshot, SnapshotSource, WatchOptions,
};
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSRunningApplication, NSWorkspace};
use objc2_application_services::{AXIsProcessTrusted, AXUIElement};

use crate::{
    app_profile::AppProfile,
    ax::{bool_attribute, collect_generic_visible_text, copy_element_attribute, string_attribute},
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
            let workspace = NSWorkspace::sharedWorkspace();
            let Some(application) = workspace.frontmostApplication() else {
                return Ok(None);
            };
            if application.isHidden() {
                return Ok(None);
            }

            let pid = application.processIdentifier();
            let app_name = application_name(&application);
            let bundle_id = application
                .bundleIdentifier()
                .map(|value| value.to_string());
            let app_profile = AppProfile::from_bundle_id(bundle_id.as_deref());
            let app_access = self.policy.access_for_bundle(bundle_id.as_deref());
            if !app_access.allows_snapshot() {
                return Ok(None);
            }
            if app_access == CaptureAccess::Metadata {
                return Ok(Some(snapshot_for_access(
                    pid,
                    app_name,
                    bundle_id,
                    app_access,
                    None,
                    None,
                    None,
                    SnapshotSource::Workspace,
                )));
            }

            ensure_trusted()?;

            let ax_application = unsafe { AXUIElement::new_application(pid) };
            let focused_window = copy_element_attribute(&ax_application, "AXFocusedWindow")
                .or_else(|_| copy_element_attribute(&ax_application, "AXMainWindow"))?;

            let Some(focused_window) = focused_window else {
                return Ok(Some(snapshot_for_access(
                    pid,
                    app_name,
                    bundle_id,
                    app_access,
                    None,
                    None,
                    None,
                    SnapshotSource::Workspace,
                )));
            };

            if bool_attribute(&focused_window, "AXMinimized")? == Some(true) {
                return Ok(None);
            }

            let bundle_id = bundle_id.filter(|value| !value.is_empty());
            let default_window_title = bundle_id.clone().unwrap_or_else(|| app_name.clone());
            let window_title = string_attribute(&focused_window, "AXTitle")?
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| default_window_title.clone());

            let mut plan = resolve_capture_plan(&CaptureContext {
                policy: &self.policy,
                bundle_id: bundle_id.as_deref(),
                app_profile,
                focused_window: &focused_window,
            })?;
            if plan.url.is_none() && plan.access == CaptureAccess::Url {
                plan.access = CaptureAccess::Metadata;
            }
            if plan.skip || !plan.access.allows_snapshot() {
                return Ok(None);
            }
            if !plan.access.allows_text() {
                return Ok(Some(snapshot_for_access(
                    pid,
                    app_name,
                    bundle_id,
                    plan.access,
                    Some(window_title),
                    plan.url,
                    None,
                    SnapshotSource::Accessibility,
                )));
            }

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

            Ok(Some(snapshot_for_access(
                pid,
                app_name,
                bundle_id,
                plan.access,
                Some(window_title),
                plan.url,
                Some(visible_text),
                SnapshotSource::Accessibility,
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

fn snapshot_for_access(
    pid: i32,
    app_name: String,
    bundle_id: Option<String>,
    access: CaptureAccess,
    window_title: Option<String>,
    url: Option<String>,
    visible_text: Option<String>,
    source: SnapshotSource,
) -> Snapshot {
    let (window_title, visible_text) =
        sanitize_snapshot_fields(&app_name, bundle_id.as_deref(), window_title, visible_text);
    let content_level = match access {
        CaptureAccess::Metadata => ContentLevel::Metadata,
        CaptureAccess::Url => ContentLevel::Url,
        CaptureAccess::Full => ContentLevel::Full,
        CaptureAccess::None => ContentLevel::Metadata,
    };

    Snapshot {
        captured_at: SystemTime::now(),
        pid,
        app_name: app_name.clone(),
        bundle_id,
        window_title: access
            .allows_text()
            .then_some(window_title.filter(|value| !value.is_empty()))
            .flatten(),
        url: access.allows_url().then_some(url).flatten(),
        visible_text: access
            .allows_text()
            .then_some(visible_text.filter(|value| !value.is_empty()))
            .flatten(),
        content_level,
        source: if access == CaptureAccess::Metadata {
            SnapshotSource::Workspace
        } else {
            source
        },
    }
}

fn application_name(application: &NSRunningApplication) -> String {
    application
        .localizedName()
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            application
                .bundleIdentifier()
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| application.processIdentifier().to_string())
}
