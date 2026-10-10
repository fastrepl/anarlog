use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct RefreshGate(Mutex<RefreshState>);

#[derive(Default)]
struct RefreshState {
    sequence: u64,
    lease: Option<(u64, Instant)>,
    retry_at: Option<Instant>,
    failures: u32,
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshPermit {
    lease_id: Option<u64>,
    retry_after_ms: u64,
}

impl RefreshGate {
    pub(crate) fn begin(&self, now: Instant) -> RefreshPermit {
        let mut state = self.0.lock().unwrap();
        let deadline = state
            .lease
            .map(|(_, deadline)| deadline)
            .into_iter()
            .chain(state.retry_at)
            .max();
        if let Some(deadline) = deadline.filter(|deadline| *deadline > now) {
            return RefreshPermit {
                lease_id: None,
                retry_after_ms: (deadline - now).as_millis().max(1) as u64,
            };
        }
        state.sequence += 1;
        let lease_id = state.sequence;
        // A closed webview must not leave every other window blocked forever.
        state.lease = Some((lease_id, now + Duration::from_secs(60)));
        RefreshPermit {
            lease_id: Some(lease_id),
            retry_after_ms: 0,
        }
    }

    pub(crate) fn finish(
        &self,
        lease_id: u64,
        status: Option<u16>,
        retry_after_ms: Option<u64>,
        now: Instant,
    ) {
        let mut state = self.0.lock().unwrap();
        if state.lease.is_none_or(|(id, _)| id != lease_id) {
            return;
        }
        state.lease = None;
        let delay = if status.is_some_and(|status| (200..300).contains(&status)) {
            state.failures = 0;
            // Let the SDK persist the rotated token before another window refreshes.
            Duration::from_secs(1)
        } else {
            let seconds = 30_u64.saturating_mul(1 << state.failures.min(4)).min(300);
            state.failures = state.failures.saturating_add(1);
            let jitter = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_millis() as u64;
            Duration::from_millis(retry_after_ms.unwrap_or(seconds * 1000 + jitter).max(1000))
        };
        state.retry_at = now.checked_add(delay);
    }

    pub(crate) fn clear(&self) {
        let mut state = self.0.lock().unwrap();
        state.lease = None;
        state.retry_at = None;
        state.failures = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_rate_limits_are_shared_and_expired_windows_cannot_release_new_leases() {
        let gate = RefreshGate::default();
        let now = Instant::now();
        let first = gate.begin(now).lease_id.unwrap();
        assert!(gate.begin(now).lease_id.is_none());
        gate.finish(first, Some(429), Some(90_000), now);
        assert!(gate.begin(now + Duration::from_secs(89)).lease_id.is_none());
        let second = gate.begin(now + Duration::from_secs(90)).lease_id.unwrap();
        let recovered = gate.begin(now + Duration::from_secs(151)).lease_id.unwrap();
        gate.finish(second, Some(200), None, now + Duration::from_secs(151));
        assert!(
            gate.begin(now + Duration::from_secs(151))
                .lease_id
                .is_none()
        );
        gate.finish(recovered, Some(200), None, now + Duration::from_secs(151));
        assert!(
            gate.begin(now + Duration::from_secs(152))
                .lease_id
                .is_some()
        );
        gate.clear();
        assert!(
            gate.begin(now + Duration::from_secs(152))
                .lease_id
                .is_some()
        );
    }
}
