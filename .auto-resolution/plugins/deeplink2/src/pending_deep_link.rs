use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::types::DeepLink;

const PENDING_DEEP_LINK_CAPACITY: usize = 32;
const PENDING_DEEP_LINK_TTL: Duration = Duration::from_secs(2 * 60);

struct Entry {
    deep_link: DeepLink,
    expires_at: Instant,
}

pub(crate) struct PendingDeepLinkState {
    entries: Mutex<VecDeque<Entry>>,
    capacity: usize,
    ttl: Duration,
}

impl Default for PendingDeepLinkState {
    fn default() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            capacity: PENDING_DEEP_LINK_CAPACITY,
            ttl: PENDING_DEEP_LINK_TTL,
        }
    }
}

impl PendingDeepLinkState {
    pub(crate) fn push(&self, deep_link: DeepLink) -> Result<(), ()> {
        self.push_at(deep_link, Instant::now())
    }

    pub(crate) fn take_all(&self) -> Result<Vec<DeepLink>, ()> {
        self.take_all_at(Instant::now())
    }

    fn push_at(&self, deep_link: DeepLink, now: Instant) -> Result<(), ()> {
        let mut entries = self.entries.lock().map_err(|_| ())?;
        prune_expired(&mut entries, now);
        while entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(Entry {
            deep_link,
            expires_at: now + self.ttl,
        });
        Ok(())
    }

    fn take_all_at(&self, now: Instant) -> Result<Vec<DeepLink>, ()> {
        let mut entries = self.entries.lock().map_err(|_| ())?;
        prune_expired(&mut entries, now);
        Ok(entries.drain(..).map(|entry| entry.deep_link).collect())
    }

    #[cfg(test)]
    fn for_test(capacity: usize, ttl: Duration) -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            capacity,
            ttl,
        }
    }
}

fn prune_expired(entries: &mut VecDeque<Entry>, now: Instant) {
    entries.retain(|entry| entry.expires_at > now);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AuthCallbackSearch, BillingRefreshSearch};

    fn deep_link() -> DeepLink {
        DeepLink::BillingRefresh(BillingRefreshSearch {})
    }

    #[test]
    fn drains_auth_tokens_once_without_debugging_them() {
        let state = PendingDeepLinkState::for_test(2, Duration::from_secs(60));
        let now = Instant::now();
        let access_token = "fake-access-secret";
        let refresh_token = "fake-refresh-secret";
        state
            .push_at(
                DeepLink::AuthCallback(AuthCallbackSearch {
                    access_token: access_token.to_string(),
                    refresh_token: refresh_token.to_string(),
                }),
                now,
            )
            .unwrap();

        let drained = state.take_all_at(now).unwrap();
        assert!(matches!(
            &drained[..],
            [DeepLink::AuthCallback(search)]
                if search.access_token == access_token && search.refresh_token == refresh_token
        ));
        let debug = format!("{drained:?}");
        assert!(!debug.contains(access_token));
        assert!(!debug.contains(refresh_token));
        assert!(state.take_all_at(now).unwrap().is_empty());
    }

    #[test]
    fn evicts_the_oldest_entry_at_capacity() {
        let state = PendingDeepLinkState::for_test(2, Duration::from_secs(60));
        let now = Instant::now();
        state.push_at(deep_link(), now).unwrap();
        state.push_at(deep_link(), now).unwrap();
        state.push_at(deep_link(), now).unwrap();

        assert_eq!(state.take_all_at(now).unwrap().len(), 2);
    }

    #[test]
    fn drops_expired_entries() {
        let state = PendingDeepLinkState::for_test(2, Duration::from_secs(5));
        let now = Instant::now();
        state.push_at(deep_link(), now).unwrap();

        assert!(
            state
                .take_all_at(now + Duration::from_secs(5))
                .unwrap()
                .is_empty()
        );
    }
}
