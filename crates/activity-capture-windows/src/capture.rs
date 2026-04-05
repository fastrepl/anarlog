#![cfg(target_os = "windows")]

use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::SystemTime,
};

use hypr_activity_capture_interface::{
    ActivityCapture, ActivityKind, Capabilities, CaptureCandidate, CaptureError, CapturePolicy,
    CaptureStream, Snapshot, SnapshotSource, SnapshotSpec, WatchOptions,
    spawn_polling_watch_stream,
};

use crate::{
    com::ensure_com_initialized,
    session::{CaptureState, clear_last_selected_session, find_active_render_session},
};

#[derive(Debug, Clone)]
pub struct WindowsCapture {
    policy: CapturePolicy,
    state: Arc<Mutex<CaptureState>>,
}

impl Default for WindowsCapture {
    fn default() -> Self {
        Self {
            policy: CapturePolicy::default(),
            state: Arc::new(Mutex::new(CaptureState::default())),
        }
    }
}

impl WindowsCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(policy: CapturePolicy) -> Self {
        Self {
            policy,
            state: Arc::new(Mutex::new(CaptureState::default())),
        }
    }

    pub(crate) fn capture_snapshot(&self) -> Result<Option<Snapshot>, CaptureError> {
        ensure_com_initialized()?;
        resolve_active_session_snapshot(self)
    }
}

impl ActivityCapture for WindowsCapture {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            can_watch: true,
            can_capture_visible_text: false,
            can_capture_browser_url: false,
            requires_accessibility_permission: false,
        }
    }

    fn snapshot(&self) -> Result<Option<Snapshot>, CaptureError> {
        let capture = self.clone();
        let (tx, rx) = mpsc::sync_channel(1);

        thread::Builder::new()
            .name("activity-capture-windows-snapshot".to_string())
            .spawn(move || {
                let _ = tx.send(capture.capture_snapshot());
            })
            .map_err(|error| CaptureError::platform(error.to_string()))?;

        rx.recv()
            .map_err(|error| CaptureError::platform(error.to_string()))?
    }

    fn watch(&self, options: WatchOptions) -> Result<CaptureStream, CaptureError> {
        let capture = self.clone();
        spawn_polling_watch_stream(
            "activity-capture-windows",
            move || capture.capture_snapshot(),
            options,
        )
    }
}

fn resolve_active_session_snapshot(
    capture: &WindowsCapture,
) -> Result<Option<Snapshot>, CaptureError> {
    let Some(candidate) = find_active_render_session(&capture.policy, &capture.state)? else {
        clear_last_selected_session(&capture.state);
        return Ok(None);
    };

    let decision = capture.policy.decision_for_candidate(&CaptureCandidate {
        app: candidate.app.clone(),
        activity_kind: ActivityKind::AudioSession,
        source: SnapshotSource::Workspace,
        browser: None,
    });
    if decision.skip || !decision.access.allows_snapshot() {
        clear_last_selected_session(&capture.state);
        return Ok(None);
    }

    Ok(Some(Snapshot::from_spec(SnapshotSpec {
        captured_at: SystemTime::now(),
        app: candidate.app,
        activity_kind: decision.activity_kind,
        access: decision.access,
        source: decision.source,
        window_title: None,
        url: decision.url,
        visible_text: None,
        text_anchor: None,
    })))
}
