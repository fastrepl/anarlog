use std::time::{SystemTime, UNIX_EPOCH};

use hypr_activity_capture as core;

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCaptureContentLevel {
    Metadata,
    Url,
    Full,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCaptureSource {
    Accessibility,
    Workspace,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCaptureAppIdKind {
    BundleId,
    ExecutablePath,
    ProcessName,
    Pid,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCaptureKind {
    ForegroundWindow,
    Browser,
    AudioSession,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCaptureTextAnchorKind {
    FocusedEdit,
    SelectedText,
    FocusedElement,
    Document,
    None,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCaptureTextAnchorConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureSnapshot {
    pub app: ActivityCaptureAppIdentity,
    pub activity_kind: ActivityCaptureKind,
    pub captured_at_ms: i64,
    pub pid: i32,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub visible_text: Option<String>,
    pub text_anchor_kind: Option<ActivityCaptureTextAnchorKind>,
    pub text_anchor_identity: Option<String>,
    pub text_anchor_text: Option<String>,
    pub text_anchor_prefix: Option<String>,
    pub text_anchor_suffix: Option<String>,
    pub text_anchor_selected_text: Option<String>,
    pub text_anchor_confidence: Option<ActivityCaptureTextAnchorConfidence>,
    pub content_level: ActivityCaptureContentLevel,
    pub source: ActivityCaptureSource,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureAppIdentity {
    pub pid: i32,
    pub app_name: String,
    pub app_id: String,
    pub app_id_kind: ActivityCaptureAppIdKind,
    pub bundle_id: Option<String>,
    pub executable_path: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCaptureErrorKind {
    PermissionDenied,
    Unsupported,
    TemporarilyUnavailable,
    Platform,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityCaptureTransitionReason {
    Started,
    Idle,
    AppChanged,
    ActivityKindChanged,
    UrlChanged,
    TitleChanged,
    TextAnchorChanged,
    ContentChanged,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureSignal {
    pub sequence: i64,
    pub occurred_at_ms: i64,
    pub reason: ActivityCaptureTransitionReason,
    pub suppressed_snapshot_count: i32,
    pub fingerprint: Option<String>,
    pub snapshot: Option<ActivityCaptureSnapshot>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureCapabilities {
    pub can_watch: bool,
    pub can_capture_visible_text: bool,
    pub can_capture_browser_url: bool,
    pub requires_accessibility_permission: bool,
}

#[derive(Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum ActivityCapturePluginEvent {
    #[serde(rename = "activityCaptureSignal")]
    Signal { signal: ActivityCaptureSignal },
    #[serde(rename = "activityCaptureError")]
    Error {
        kind: ActivityCaptureErrorKind,
        message: String,
    },
}

impl From<core::Capabilities> for ActivityCaptureCapabilities {
    fn from(value: core::Capabilities) -> Self {
        Self {
            can_watch: value.can_watch,
            can_capture_visible_text: value.can_capture_visible_text,
            can_capture_browser_url: value.can_capture_browser_url,
            requires_accessibility_permission: value.requires_accessibility_permission,
        }
    }
}

impl From<core::ContentLevel> for ActivityCaptureContentLevel {
    fn from(value: core::ContentLevel) -> Self {
        match value {
            core::ContentLevel::Metadata => Self::Metadata,
            core::ContentLevel::Url => Self::Url,
            core::ContentLevel::Full => Self::Full,
        }
    }
}

impl From<core::SnapshotSource> for ActivityCaptureSource {
    fn from(value: core::SnapshotSource) -> Self {
        match value {
            core::SnapshotSource::Accessibility => Self::Accessibility,
            core::SnapshotSource::Workspace => Self::Workspace,
        }
    }
}

impl From<core::AppIdKind> for ActivityCaptureAppIdKind {
    fn from(value: core::AppIdKind) -> Self {
        match value {
            core::AppIdKind::BundleId => Self::BundleId,
            core::AppIdKind::ExecutablePath => Self::ExecutablePath,
            core::AppIdKind::ProcessName => Self::ProcessName,
            core::AppIdKind::Pid => Self::Pid,
        }
    }
}

impl From<core::ActivityKind> for ActivityCaptureKind {
    fn from(value: core::ActivityKind) -> Self {
        match value {
            core::ActivityKind::ForegroundWindow => Self::ForegroundWindow,
            core::ActivityKind::Browser => Self::Browser,
            core::ActivityKind::AudioSession => Self::AudioSession,
        }
    }
}

impl From<core::TextAnchorKind> for ActivityCaptureTextAnchorKind {
    fn from(value: core::TextAnchorKind) -> Self {
        match value {
            core::TextAnchorKind::FocusedEdit => Self::FocusedEdit,
            core::TextAnchorKind::SelectedText => Self::SelectedText,
            core::TextAnchorKind::FocusedElement => Self::FocusedElement,
            core::TextAnchorKind::Document => Self::Document,
            core::TextAnchorKind::None => Self::None,
        }
    }
}

impl From<core::TextAnchorConfidence> for ActivityCaptureTextAnchorConfidence {
    fn from(value: core::TextAnchorConfidence) -> Self {
        match value {
            core::TextAnchorConfidence::High => Self::High,
            core::TextAnchorConfidence::Medium => Self::Medium,
            core::TextAnchorConfidence::Low => Self::Low,
        }
    }
}

impl From<core::AppIdentity> for ActivityCaptureAppIdentity {
    fn from(value: core::AppIdentity) -> Self {
        Self {
            pid: value.pid,
            app_name: value.app_name,
            app_id: value.app_id,
            app_id_kind: value.app_id_kind.into(),
            bundle_id: value.bundle_id,
            executable_path: value.executable_path,
        }
    }
}

impl From<core::Snapshot> for ActivityCaptureSnapshot {
    fn from(value: core::Snapshot) -> Self {
        Self {
            app: value.app.clone().into(),
            activity_kind: value.activity_kind.into(),
            captured_at_ms: system_time_to_unix_ms(value.captured_at),
            pid: value.pid,
            app_name: value.app_name,
            bundle_id: value.bundle_id,
            window_title: value.window_title,
            url: value.url,
            visible_text: value.visible_text,
            text_anchor_kind: value.text_anchor_kind.map(Into::into),
            text_anchor_identity: value.text_anchor_identity,
            text_anchor_text: value.text_anchor_text,
            text_anchor_prefix: value.text_anchor_prefix,
            text_anchor_suffix: value.text_anchor_suffix,
            text_anchor_selected_text: value.text_anchor_selected_text,
            text_anchor_confidence: value.text_anchor_confidence.map(Into::into),
            content_level: value.content_level.into(),
            source: value.source.into(),
        }
    }
}

impl From<core::Transition> for ActivityCaptureSignal {
    fn from(value: core::Transition) -> Self {
        let occurred_at_ms = value
            .current
            .as_ref()
            .map(|event| system_time_to_unix_ms(event.started_at))
            .or_else(|| {
                value
                    .previous
                    .as_ref()
                    .map(|event| system_time_to_unix_ms(event.ended_at))
            })
            .unwrap_or_default();
        let fingerprint = value
            .current
            .as_ref()
            .map(|event| event.fingerprint.clone());
        Self {
            occurred_at_ms,
            reason: value.reason.into(),
            sequence: value.sequence.min(i64::MAX as u64) as i64,
            suppressed_snapshot_count: value.suppressed_snapshot_count.min(i32::MAX as u32) as i32,
            fingerprint,
            snapshot: value.current.map(|event| event.snapshot.into()),
        }
    }
}

impl From<core::TransitionReason> for ActivityCaptureTransitionReason {
    fn from(value: core::TransitionReason) -> Self {
        match value {
            core::TransitionReason::Started => Self::Started,
            core::TransitionReason::Idle => Self::Idle,
            core::TransitionReason::AppChanged => Self::AppChanged,
            core::TransitionReason::ActivityKindChanged => Self::ActivityKindChanged,
            core::TransitionReason::UrlChanged => Self::UrlChanged,
            core::TransitionReason::TitleChanged => Self::TitleChanged,
            core::TransitionReason::TextAnchorChanged => Self::TextAnchorChanged,
            core::TransitionReason::ContentChanged => Self::ContentChanged,
        }
    }
}

impl From<core::CaptureErrorKind> for ActivityCaptureErrorKind {
    fn from(value: core::CaptureErrorKind) -> Self {
        match value {
            core::CaptureErrorKind::PermissionDenied => Self::PermissionDenied,
            core::CaptureErrorKind::Unsupported => Self::Unsupported,
            core::CaptureErrorKind::TemporarilyUnavailable => Self::TemporarilyUnavailable,
            core::CaptureErrorKind::Platform => Self::Platform,
        }
    }
}

fn system_time_to_unix_ms(value: SystemTime) -> i64 {
    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(i64::MAX as u128) as i64,
        Err(error) => -(error.duration().as_millis().min(i64::MAX as u128) as i64),
    }
}
