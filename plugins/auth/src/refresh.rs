use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct RefreshGate(Mutex<RefreshState>);

#[derive(Default)]
struct RefreshState {
    sequence: u64,
    credentials: HashMap<[u8; 32], CredentialRefresh>,
}

#[derive(Default)]
struct CredentialRefresh {
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
    pub(crate) fn begin(&self, refresh_token: &str, now: Instant) -> RefreshPermit {
        let identity: [u8; 32] = Sha256::digest(refresh_token.as_bytes()).into();
        let mut state = self.0.lock().unwrap();
        // Keep failure history for an hour after cooldown, including interleaved accounts.
        state.credentials.retain(|_, credential| {
            credential.deadline().is_some_and(|deadline| {
                deadline > now
                    || (credential.failures > 0
                        && now.saturating_duration_since(deadline) < Duration::from_secs(3600))
            })
        });
        let deadline = state
            .credentials
            .get(&identity)
            .and_then(CredentialRefresh::deadline);
        if let Some(deadline) = deadline.filter(|deadline| *deadline > now) {
            return RefreshPermit {
                lease_id: None,
                retry_after_ms: (deadline - now).as_millis().max(1) as u64,
            };
        }
        state.sequence += 1;
        let lease_id = state.sequence;
        // A closed webview must not leave every other window blocked forever.
        state.credentials.entry(identity).or_default().lease =
            Some((lease_id, now + Duration::from_secs(60)));
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
        let Some(credential) = state
            .credentials
            .values_mut()
            .find(|credential| credential.lease.is_some_and(|(id, _)| id == lease_id))
        else {
            return;
        };
        credential.lease = None;
        let delay = if status.is_some_and(|status| (200..300).contains(&status)) {
            credential.failures = 0;
            // Let the SDK persist the rotated token before another window refreshes.
            Duration::from_secs(1)
        } else {
            let seconds = 30_u64
                .saturating_mul(1 << credential.failures.min(4))
                .min(300);
            credential.failures = credential.failures.saturating_add(1);
            let jitter = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_millis() as u64;
            Duration::from_millis(retry_after_ms.unwrap_or(seconds * 1000 + jitter).max(1000))
        };
        credential.retry_at = now.checked_add(delay);
    }

    pub(crate) fn clear(&self) {
        let mut state = self.0.lock().unwrap();
        state.credentials.clear();
    }
}

impl CredentialRefresh {
    fn deadline(&self) -> Option<Instant> {
        self.lease
            .map(|(_, deadline)| deadline)
            .into_iter()
            .chain(self.retry_at)
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaved_credentials_keep_backoff_until_failure_history_expires() {
        let gate = RefreshGate::default();
        let now = Instant::now();
        let first = gate.begin("account-a", now).lease_id.unwrap();
        gate.finish(first, None, None, now);
        let later = now + Duration::from_secs(32);
        let other = gate.begin("account-b", later).lease_id.unwrap();
        gate.finish(other, Some(200), None, later);
        let retry = gate.begin("account-a", later).lease_id.unwrap();
        gate.finish(retry, None, None, later);
        assert!(
            gate.begin("account-a", later + Duration::from_secs(59))
                .lease_id
                .is_none()
        );
        let expired = now + Duration::from_secs(4000);
        let fresh = gate.begin("account-a", expired).lease_id.unwrap();
        gate.finish(fresh, None, None, expired);
        assert!(
            gate.begin("account-a", expired + Duration::from_secs(32))
                .lease_id
                .is_some()
        );
    }

    #[test]
    fn refresh_rate_limits_are_shared_and_expired_windows_cannot_release_new_leases() {
        let gate = RefreshGate::default();
        let now = Instant::now();
        let first = gate.begin("first-token", now).lease_id.unwrap();
        assert!(gate.begin("first-token", now).lease_id.is_none());
        gate.finish(first, Some(429), Some(90_000), now);
        let other_account = gate.begin("other-account-token", now).lease_id.unwrap();
        gate.finish(other_account, Some(200), None, now);
        let rotated = gate.begin("rotated-token", now).lease_id.unwrap();
        gate.finish(rotated, Some(200), None, now);
        let stale = gate.begin("stale-token", now).lease_id.unwrap();
        gate.finish(stale, Some(200), None, now);
        let stale_retry = gate
            .begin("stale-token", now + Duration::from_secs(1))
            .lease_id
            .unwrap();
        gate.finish(stale_retry, Some(400), None, now + Duration::from_secs(1));
        assert!(
            gate.begin("rotated-token", now + Duration::from_secs(1))
                .lease_id
                .is_some()
        );
        assert!(
            gate.begin("first-token", now + Duration::from_secs(89))
                .lease_id
                .is_none()
        );
        let second = gate
            .begin("first-token", now + Duration::from_secs(90))
            .lease_id
            .unwrap();
        let recovered = gate
            .begin("first-token", now + Duration::from_secs(151))
            .lease_id
            .unwrap();
        gate.finish(second, Some(200), None, now + Duration::from_secs(151));
        assert!(
            gate.begin("first-token", now + Duration::from_secs(151))
                .lease_id
                .is_none()
        );
        gate.finish(recovered, Some(200), None, now + Duration::from_secs(151));
        assert!(
            gate.begin("first-token", now + Duration::from_secs(152))
                .lease_id
                .is_some()
        );
        gate.clear();
        assert!(
            gate.begin("first-token", now + Duration::from_secs(152))
                .lease_id
                .is_some()
        );
    }
}
