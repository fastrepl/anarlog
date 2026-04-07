use std::sync::{Arc, Mutex};

use hypr_activity_capture_interface::{ActivityKind, Snapshot, Transition, TransitionReason};
use hypr_screen_core::{WindowCaptureTarget, WindowContextCaptureOptions, WindowContextImage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityScreenshotTarget {
    pub pid: u32,
    pub app_name: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityScreenshotCapture {
    pub fingerprint: String,
    pub reason: TransitionReason,
    pub scheduled_at_ms: i64,
    pub captured_at_ms: i64,
    pub target: ActivityScreenshotTarget,
    pub image: WindowContextImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StableSegmentScreenshotPolicyConfig {
    pub dwell_ms: u64,
}

impl Default for StableSegmentScreenshotPolicyConfig {
    fn default() -> Self {
        Self { dwell_ms: 10_000 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCapture {
    pub pending_id: u64,
    pub occurrence_id: u64,
    pub fingerprint: String,
    pub reason: TransitionReason,
    pub scheduled_at_ms: i64,
    pub due_at_ms: i64,
    pub target: ActivityScreenshotTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyUpdate {
    None,
    CancelPending,
    Schedule(PendingCapture),
    CancelAndSchedule(PendingCapture),
}

pub trait ScreenshotPolicy: Send {
    fn on_transition(&mut self, transition: &Transition, now_ms: i64) -> PolicyUpdate;

    fn fire_pending_capture(&mut self, pending_id: u64, now_ms: i64) -> Option<PendingCapture>;
}

pub trait ScreenshotSink: Send + Sync {
    fn store(&self, capture: ActivityScreenshotCapture);
}

pub trait ScreenshotCapturer: Send + Sync {
    fn capture(&self, target: &ActivityScreenshotTarget) -> Result<WindowContextImage, String>;
}

#[derive(Default)]
pub struct LatestCaptureState {
    latest: Mutex<Option<ActivityScreenshotCapture>>,
}

impl LatestCaptureState {
    pub fn set(&self, capture: ActivityScreenshotCapture) {
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(capture);
    }

    pub fn latest(&self) -> Option<ActivityScreenshotCapture> {
        self.latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

pub struct LatestCaptureSink {
    state: Arc<LatestCaptureState>,
}

impl LatestCaptureSink {
    pub fn new(state: Arc<LatestCaptureState>) -> Self {
        Self { state }
    }
}

impl ScreenshotSink for LatestCaptureSink {
    fn store(&self, capture: ActivityScreenshotCapture) {
        self.state.set(capture);
    }
}

#[derive(Default)]
pub struct ScreenCoreCapturer;

impl ScreenshotCapturer for ScreenCoreCapturer {
    fn capture(&self, target: &ActivityScreenshotTarget) -> Result<WindowContextImage, String> {
        hypr_screen_core::capture_target_window_context(
            &WindowCaptureTarget {
                pid: target.pid,
                app_name: Some(target.app_name.clone()),
                title: target.title.clone(),
            },
            WindowContextCaptureOptions::default(),
        )
        .map_err(|error| error.to_string())
    }
}

pub struct ActivityScreenshotCoordinator {
    policy: Box<dyn ScreenshotPolicy>,
    sink: Arc<dyn ScreenshotSink>,
    capturer: Arc<dyn ScreenshotCapturer>,
}

impl ActivityScreenshotCoordinator {
    pub fn new(
        policy: Box<dyn ScreenshotPolicy>,
        sink: Arc<dyn ScreenshotSink>,
        capturer: Arc<dyn ScreenshotCapturer>,
    ) -> Self {
        Self {
            policy,
            sink,
            capturer,
        }
    }

    pub fn handle_transition(&mut self, transition: &Transition, now_ms: i64) -> PolicyUpdate {
        self.policy.on_transition(transition, now_ms)
    }

    pub fn fire_pending_capture(&mut self, pending_id: u64, now_ms: i64) -> Result<bool, String> {
        let Some(pending) = self.policy.fire_pending_capture(pending_id, now_ms) else {
            return Ok(false);
        };

        let image = self.capturer.capture(&pending.target)?;
        let captured_at_ms = image.captured_at_ms;
        self.sink.store(ActivityScreenshotCapture {
            fingerprint: pending.fingerprint,
            reason: pending.reason,
            scheduled_at_ms: pending.scheduled_at_ms,
            captured_at_ms,
            target: pending.target,
            image,
        });

        Ok(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveOccurrence {
    occurrence_id: u64,
    fingerprint: String,
    capture_attempted: bool,
}

pub struct StableSegmentScreenshotPolicy {
    config: StableSegmentScreenshotPolicyConfig,
    next_occurrence_id: u64,
    next_pending_id: u64,
    active: Option<ActiveOccurrence>,
    pending: Option<PendingCapture>,
}

impl Default for StableSegmentScreenshotPolicy {
    fn default() -> Self {
        Self::new(StableSegmentScreenshotPolicyConfig::default())
    }
}

impl StableSegmentScreenshotPolicy {
    pub fn new(config: StableSegmentScreenshotPolicyConfig) -> Self {
        Self {
            config,
            next_occurrence_id: 0,
            next_pending_id: 0,
            active: None,
            pending: None,
        }
    }

    fn clear_pending(&mut self) -> PolicyUpdate {
        if self.pending.take().is_some() {
            PolicyUpdate::CancelPending
        } else {
            PolicyUpdate::None
        }
    }

    fn replace_pending(&mut self, pending: PendingCapture) -> PolicyUpdate {
        let had_pending = self.pending.replace(pending.clone()).is_some();
        if had_pending {
            PolicyUpdate::CancelAndSchedule(pending)
        } else {
            PolicyUpdate::Schedule(pending)
        }
    }
}

impl ScreenshotPolicy for StableSegmentScreenshotPolicy {
    fn on_transition(&mut self, transition: &Transition, now_ms: i64) -> PolicyUpdate {
        if matches!(transition.reason, TransitionReason::Idle) || transition.current.is_none() {
            self.active = None;
            return self.clear_pending();
        }

        let Some(current) = transition.current.as_ref() else {
            self.active = None;
            return self.clear_pending();
        };
        let snapshot = &current.snapshot;
        let fingerprint = current.fingerprint.clone();

        self.next_occurrence_id += 1;
        let occurrence_id = self.next_occurrence_id;
        self.active = Some(ActiveOccurrence {
            occurrence_id,
            fingerprint: fingerprint.clone(),
            capture_attempted: false,
        });

        if !is_supported_kind(snapshot.activity_kind) || !is_eligible_reason(transition.reason) {
            return self.clear_pending();
        }

        let Some(target) = target_from_snapshot(snapshot) else {
            return self.clear_pending();
        };

        self.next_pending_id += 1;
        let dwell_ms = self.config.dwell_ms.min(i64::MAX as u64) as i64;
        let pending = PendingCapture {
            pending_id: self.next_pending_id,
            occurrence_id,
            fingerprint,
            reason: transition.reason,
            scheduled_at_ms: now_ms,
            due_at_ms: now_ms.saturating_add(dwell_ms),
            target,
        };

        self.replace_pending(pending)
    }

    fn fire_pending_capture(&mut self, pending_id: u64, now_ms: i64) -> Option<PendingCapture> {
        let pending = match self.pending.as_ref() {
            Some(pending) if pending.pending_id == pending_id && now_ms >= pending.due_at_ms => {
                pending.clone()
            }
            _ => return None,
        };

        self.pending = None;

        let Some(active) = self.active.as_mut() else {
            return None;
        };
        if active.occurrence_id != pending.occurrence_id
            || active.fingerprint != pending.fingerprint
            || active.capture_attempted
        {
            return None;
        }

        active.capture_attempted = true;
        Some(pending)
    }
}

fn is_eligible_reason(reason: TransitionReason) -> bool {
    matches!(
        reason,
        TransitionReason::Started
            | TransitionReason::AppChanged
            | TransitionReason::ActivityKindChanged
            | TransitionReason::UrlChanged
            | TransitionReason::TitleChanged
    )
}

fn is_supported_kind(kind: ActivityKind) -> bool {
    matches!(kind, ActivityKind::ForegroundWindow | ActivityKind::Browser)
}

fn target_from_snapshot(snapshot: &Snapshot) -> Option<ActivityScreenshotTarget> {
    Some(ActivityScreenshotTarget {
        pid: u32::try_from(snapshot.pid).ok()?,
        app_name: snapshot.app_name.clone(),
        title: snapshot
            .window_title
            .clone()
            .filter(|value| !value.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_activity_capture_interface::{
        AppIdKind, AppIdentity, CaptureAccess, Event, SnapshotSource, SnapshotSpec,
    };
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[derive(Default)]
    struct RecordingSink {
        captures: Mutex<Vec<ActivityScreenshotCapture>>,
    }

    impl RecordingSink {
        fn captures(&self) -> Vec<ActivityScreenshotCapture> {
            self.captures
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        }
    }

    impl ScreenshotSink for RecordingSink {
        fn store(&self, capture: ActivityScreenshotCapture) {
            self.captures
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(capture);
        }
    }

    struct FakeCapturer {
        result: Result<WindowContextImage, String>,
    }

    impl ScreenshotCapturer for FakeCapturer {
        fn capture(
            &self,
            _target: &ActivityScreenshotTarget,
        ) -> Result<WindowContextImage, String> {
            self.result.clone()
        }
    }

    fn snapshot(kind: ActivityKind, pid: i32, title: &str) -> Snapshot {
        let captured_at = UNIX_EPOCH + Duration::from_secs(1);
        Snapshot::from_spec(SnapshotSpec {
            captured_at,
            app: AppIdentity {
                pid,
                app_name: "Code".to_string(),
                app_id: "com.microsoft.VSCode".to_string(),
                app_id_kind: AppIdKind::BundleId,
                bundle_id: Some("com.microsoft.VSCode".to_string()),
                executable_path: None,
            },
            activity_kind: kind,
            access: CaptureAccess::Full,
            source: SnapshotSource::Accessibility,
            window_title: Some(title.to_string()),
            url: None,
            visible_text: None,
            text_anchor: None,
        })
    }

    fn transition(reason: TransitionReason, fingerprint: &str, snapshot: Snapshot) -> Transition {
        Transition {
            previous: None,
            current: Some(Event {
                started_at: SystemTime::UNIX_EPOCH,
                ended_at: SystemTime::UNIX_EPOCH,
                fingerprint: fingerprint.to_string(),
                snapshot,
            }),
            reason,
            sequence: 1,
            suppressed_snapshot_count: 0,
        }
    }

    fn idle_transition() -> Transition {
        Transition {
            previous: None,
            current: None,
            reason: TransitionReason::Idle,
            sequence: 2,
            suppressed_snapshot_count: 0,
        }
    }

    fn fake_window_context_image() -> WindowContextImage {
        WindowContextImage {
            image_bytes: vec![1, 2, 3],
            mime_type: "image/webp".to_string(),
            captured_at_ms: 12_345,
            width: 320,
            height: 200,
            strategy: hypr_screen_core::CaptureStrategy::WindowWithContext,
            crop: hypr_screen_core::CaptureRect {
                x: 0,
                y: 0,
                width: 320,
                height: 200,
            },
            window: hypr_screen_core::WindowMetadata {
                id: 1,
                pid: 42,
                app_name: "Code".to_string(),
                title: "main.rs".to_string(),
                rect: hypr_screen_core::CaptureRect {
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 200,
                },
            },
        }
    }

    #[test]
    fn policy_started_schedules_delayed_capture() {
        let mut policy = StableSegmentScreenshotPolicy::default();
        let update = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        );

        match update {
            PolicyUpdate::Schedule(pending) => {
                assert_eq!(pending.fingerprint, "fp1");
                assert_eq!(pending.reason, TransitionReason::Started);
                assert_eq!(pending.scheduled_at_ms, 1_000);
                assert_eq!(pending.due_at_ms, 11_000);
            }
            other => panic!("expected schedule, got {other:?}"),
        }
    }

    #[test]
    fn policy_ignores_text_and_content_changes() {
        let mut policy = StableSegmentScreenshotPolicy::default();
        let text_update = policy.on_transition(
            &transition(
                TransitionReason::TextAnchorChanged,
                "fp2",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        );
        let content_update = policy.on_transition(
            &transition(
                TransitionReason::ContentChanged,
                "fp3",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            2_000,
        );

        assert_eq!(text_update, PolicyUpdate::None);
        assert_eq!(content_update, PolicyUpdate::None);
    }

    #[test]
    fn policy_second_transition_cancels_first_pending_capture() {
        let mut policy = StableSegmentScreenshotPolicy::default();
        let first = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        );
        let second = policy.on_transition(
            &transition(
                TransitionReason::AppChanged,
                "fp2",
                snapshot(ActivityKind::ForegroundWindow, 43, "lib.rs"),
            ),
            2_000,
        );

        assert!(matches!(first, PolicyUpdate::Schedule(_)));
        assert!(matches!(second, PolicyUpdate::CancelAndSchedule(_)));
    }

    #[test]
    fn policy_timer_fire_same_fingerprint_captures_once() {
        let mut policy = StableSegmentScreenshotPolicy::default();
        let pending = match policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        ) {
            PolicyUpdate::Schedule(pending) => pending,
            other => panic!("expected schedule, got {other:?}"),
        };

        assert!(
            policy
                .fire_pending_capture(pending.pending_id, 10_999)
                .is_none()
        );
        assert!(
            policy
                .fire_pending_capture(pending.pending_id, 11_000)
                .is_some()
        );
        assert!(
            policy
                .fire_pending_capture(pending.pending_id, 12_000)
                .is_none()
        );
    }

    #[test]
    fn policy_timer_fire_after_fingerprint_change_does_not_capture() {
        let mut policy = StableSegmentScreenshotPolicy::default();
        let pending = match policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        ) {
            PolicyUpdate::Schedule(pending) => pending,
            other => panic!("expected schedule, got {other:?}"),
        };
        let update = policy.on_transition(
            &transition(
                TransitionReason::TitleChanged,
                "fp2",
                snapshot(ActivityKind::ForegroundWindow, 42, "other.rs"),
            ),
            2_000,
        );

        assert!(matches!(update, PolicyUpdate::CancelAndSchedule(_)));
        assert!(
            policy
                .fire_pending_capture(pending.pending_id, 11_000)
                .is_none()
        );
    }

    #[test]
    fn policy_reenter_same_fingerprint_creates_new_occurrence() {
        let mut policy =
            StableSegmentScreenshotPolicy::new(StableSegmentScreenshotPolicyConfig { dwell_ms: 0 });
        let first = match policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        ) {
            PolicyUpdate::Schedule(pending) => pending,
            other => panic!("expected schedule, got {other:?}"),
        };
        assert!(
            policy
                .fire_pending_capture(first.pending_id, 1_000)
                .is_some()
        );
        let idle_update = policy.on_transition(&idle_transition(), 2_000);
        let second = match policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            3_000,
        ) {
            PolicyUpdate::Schedule(pending) => pending,
            other => panic!("expected schedule, got {other:?}"),
        };

        assert_eq!(idle_update, PolicyUpdate::None);
        assert_ne!(first.occurrence_id, second.occurrence_id);
        assert!(
            policy
                .fire_pending_capture(second.pending_id, 3_000)
                .is_some()
        );
    }

    #[test]
    fn policy_idle_clears_pending() {
        let mut policy = StableSegmentScreenshotPolicy::default();
        let _ = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        );
        let update = policy.on_transition(&idle_transition(), 2_000);

        assert_eq!(update, PolicyUpdate::CancelPending);
    }

    #[test]
    fn coordinator_due_capture_writes_to_sink() {
        let sink = Arc::new(RecordingSink::default());
        let mut coordinator = ActivityScreenshotCoordinator::new(
            Box::new(StableSegmentScreenshotPolicy::default()),
            sink.clone(),
            Arc::new(FakeCapturer {
                result: Ok(fake_window_context_image()),
            }),
        );
        let pending = match coordinator.handle_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        ) {
            PolicyUpdate::Schedule(pending) => pending,
            other => panic!("expected schedule, got {other:?}"),
        };

        coordinator
            .fire_pending_capture(pending.pending_id, 11_000)
            .unwrap();

        let captures = sink.captures();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].fingerprint, "fp1");
        assert_eq!(captures[0].target.pid, 42);
    }

    #[test]
    fn coordinator_non_window_activity_never_captures() {
        let sink = Arc::new(RecordingSink::default());
        let mut coordinator = ActivityScreenshotCoordinator::new(
            Box::new(StableSegmentScreenshotPolicy::default()),
            sink.clone(),
            Arc::new(FakeCapturer {
                result: Ok(fake_window_context_image()),
            }),
        );
        let update = coordinator.handle_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::AudioSession, 42, "main.rs"),
            ),
            1_000,
        );

        assert_eq!(update, PolicyUpdate::None);
        assert!(sink.captures().is_empty());
    }

    #[test]
    fn coordinator_capture_errors_do_not_store() {
        let sink = Arc::new(RecordingSink::default());
        let mut coordinator = ActivityScreenshotCoordinator::new(
            Box::new(StableSegmentScreenshotPolicy::default()),
            sink.clone(),
            Arc::new(FakeCapturer {
                result: Err("capture failed".to_string()),
            }),
        );
        let pending = match coordinator.handle_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        ) {
            PolicyUpdate::Schedule(pending) => pending,
            other => panic!("expected schedule, got {other:?}"),
        };

        assert_eq!(
            coordinator.fire_pending_capture(pending.pending_id, 11_000),
            Err("capture failed".to_string())
        );
        assert!(sink.captures().is_empty());
    }
}
