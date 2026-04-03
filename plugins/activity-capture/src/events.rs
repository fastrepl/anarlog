use std::time::{SystemTime, UNIX_EPOCH};

use hypr_activity_capture_interface as core;

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

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureSnapshot {
    pub captured_at_ms: i64,
    pub pid: i32,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub visible_text: Option<String>,
    pub content_level: ActivityCaptureContentLevel,
    pub source: ActivityCaptureSource,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureEvent {
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
    pub fingerprint: String,
    pub snapshot: ActivityCaptureSnapshot,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureTransition {
    pub previous: Option<ActivityCaptureEvent>,
    pub current: Option<ActivityCaptureEvent>,
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
    #[serde(rename = "activityCaptureTransition")]
    Transition {
        transition: ActivityCaptureTransition,
    },
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

impl From<core::Snapshot> for ActivityCaptureSnapshot {
    fn from(value: core::Snapshot) -> Self {
        Self {
            captured_at_ms: system_time_to_unix_ms(value.captured_at),
            pid: value.pid,
            app_name: value.app_name,
            bundle_id: value.bundle_id,
            window_title: value.window_title,
            url: value.url,
            visible_text: value.visible_text,
            content_level: value.content_level.into(),
            source: value.source.into(),
        }
    }
}

impl From<core::Event> for ActivityCaptureEvent {
    fn from(value: core::Event) -> Self {
        Self {
            started_at_ms: system_time_to_unix_ms(value.started_at),
            ended_at_ms: system_time_to_unix_ms(value.ended_at),
            fingerprint: value.fingerprint,
            snapshot: value.snapshot.into(),
        }
    }
}

impl From<core::Transition> for ActivityCaptureTransition {
    fn from(value: core::Transition) -> Self {
        Self {
            previous: value.previous.map(Into::into),
            current: value.current.map(Into::into),
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
