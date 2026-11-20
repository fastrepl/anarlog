use std::time::{Duration, Instant};

use anlg_audio::meeting::{CameraBackgroundRequest, CameraOutputReceipt};

use crate::helper::{Snapshot, now_ms};
use crate::prefs::CameraBackground;

/// How long Anarlog waits for Anarlog Camera's receipt before it leaves the app's effect on: the
/// agent reads `state.json` twice a second and rewrites its status at least every 2 s.
pub const RECEIPT_WAIT: Duration = Duration::from_secs(4);
const RECEIPT_POLL: Duration = Duration::from_millis(250);

/// Whether `snapshot` proves that frames hiding the room for exactly `request_id` went out to
/// meeting apps, in a status the agent wrote at or after `since_ms`: the agent was last told
/// `expected` under `request_id` (no newer request), and its `protected_background_id` names that
/// request. A Background that only was asked for, an older request, a stalled or older agent, or
/// a status from before the wait began proves nothing. Frames a meeting app already buffered are
/// beyond it.
pub fn confirms(
    snapshot: &Snapshot,
    expected: CameraBackground,
    request_id: &str,
    since_ms: u64,
) -> bool {
    if !expected.hides_room()
        || !snapshot.running
        || snapshot.background != expected
        || snapshot.background_id != request_id
    {
        return false;
    }
    let (Some(status), Some(written)) = (&snapshot.status, snapshot.status_written_at_ms) else {
        return false;
    };
    let Some(protected) = &status.protected_background_id else {
        return false;
    };
    let request = CameraBackgroundRequest {
        request_id: request_id.to_string(),
        background: Some(
            expected.wire()["kind"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        ),
    };
    CameraOutputReceipt {
        request_id: protected.clone(),
        emitted_at_ms: written,
    }
    .confirms(&request, since_ms)
}

/// Waits up to `wait` for [`confirms`]; false on timeout or once `cancelled` says so.
pub fn wait_for(
    snapshot: impl Fn() -> Snapshot,
    expected: CameraBackground,
    request_id: &str,
    wait: Duration,
    cancelled: &dyn Fn() -> bool,
) -> bool {
    if !expected.hides_room() {
        return false;
    }
    let since = now_ms();
    let deadline = Instant::now() + wait;
    loop {
        if cancelled() {
            return false;
        }
        if confirms(&snapshot(), expected, request_id, since) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(RECEIPT_POLL.min(deadline.saturating_duration_since(Instant::now())));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::helper::{AgentStatus, CameraAccess};
    use crate::prefs::CameraScene;

    const DUSK: CameraBackground = CameraBackground::Scene {
        scene: CameraScene::Dusk,
    };

    fn agent(protected: Option<&str>, written: Option<u64>) -> Snapshot {
        Snapshot {
            running: true,
            status: Some(AgentStatus {
                camera_installed: true,
                consumers: vec!["us.zoom.xos".into()],
                recording: true,
                badge: true,
                camera_permission: CameraAccess::Allowed,
                source_camera: None,
                problem: None,
                background: Some("blur".into()),
                protected_background_id: protected.map(str::to_string),
            }),
            extension: None,
            agent: None,
            camera_found: None,
            camera_permission: Some(CameraAccess::Allowed),
            background: CameraBackground::Blur,
            background_id: "b-2".into(),
            status_written_at_ms: written,
        }
    }

    #[test]
    fn never_counts_a_background_the_agent_was_only_asked_to_draw() {
        assert!(!confirms(
            &agent(None, Some(10)),
            CameraBackground::Blur,
            "b-2",
            5
        ));
    }

    #[test]
    fn counts_frames_sent_for_exactly_the_current_request() {
        assert!(confirms(
            &agent(Some("b-2"), Some(10)),
            CameraBackground::Blur,
            "b-2",
            5
        ));
        // A receipt from an earlier request, even for the same blur.
        assert!(!confirms(
            &agent(Some("b-1"), Some(10)),
            CameraBackground::Blur,
            "b-1",
            5
        ));
        assert!(!confirms(
            &agent(Some("b-1"), Some(10)),
            CameraBackground::Blur,
            "b-2",
            5
        ));
    }

    #[test]
    fn never_trusts_a_status_written_before_the_wait_began() {
        assert!(!confirms(
            &agent(Some("b-2"), Some(4)),
            CameraBackground::Blur,
            "b-2",
            5
        ));
    }

    #[test]
    fn fails_closed_when_the_request_or_the_agent_changed() {
        // Another Background was asked for meanwhile.
        assert!(!confirms(&agent(Some("b-2"), Some(10)), DUSK, "b-2", 5));
        let mut gone = agent(Some("b-2"), Some(10));
        gone.running = false;
        assert!(!confirms(&gone, CameraBackground::Blur, "b-2", 5));
        // No Background never hides the room, whatever the agent says.
        let mut plain = agent(Some("b-2"), Some(10));
        plain.background = CameraBackground::None;
        assert!(!confirms(&plain, CameraBackground::None, "b-2", 5));
    }

    #[test]
    fn waits_for_the_receipt_of_a_background_just_asked_for() {
        let current = Mutex::new(agent(Some("b-1"), Some(0)));
        let started = Instant::now();
        let hidden = std::thread::scope(|scope| {
            let waiting = scope.spawn(|| {
                wait_for(
                    || current.lock().unwrap().clone(),
                    CameraBackground::Blur,
                    "b-2",
                    Duration::from_secs(3),
                    &|| false,
                )
            });
            std::thread::sleep(Duration::from_millis(300));
            *current.lock().unwrap() = agent(Some("b-2"), Some(now_ms() + 1));
            waiting.join().unwrap()
        });
        assert!(hidden);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn stops_waiting_when_cancelled() {
        let started = Instant::now();
        assert!(!wait_for(
            || agent(Some("b-2"), Some(now_ms() + 1)),
            CameraBackground::Blur,
            "b-2",
            Duration::from_secs(10),
            &|| true,
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
