use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::types::ShareOpenRequest;

const PENDING_SHARE_OPEN_CAPACITY: usize = 32;
const PENDING_SHARE_OPEN_TTL: Duration = Duration::from_secs(2 * 60);

struct Entry {
    id: String,
    request: ShareOpenRequest,
    expires_at: Instant,
}

pub(crate) struct PendingShareOpenState {
    entries: Mutex<VecDeque<Entry>>,
    capacity: usize,
    ttl: Duration,
}

impl Default for PendingShareOpenState {
    fn default() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            capacity: PENDING_SHARE_OPEN_CAPACITY,
            ttl: PENDING_SHARE_OPEN_TTL,
        }
    }
}

impl PendingShareOpenState {
    pub(crate) fn push(&self, request: ShareOpenRequest) -> Result<String, ()> {
        self.push_at(request, Instant::now())
    }

    pub(crate) fn list(&self) -> Result<Vec<String>, ()> {
        self.list_at(Instant::now())
    }

    pub(crate) fn take(&self, id: &str) -> Result<Option<ShareOpenRequest>, ()> {
        if !is_canonical_uuid(id) {
            return Ok(None);
        }
        self.take_at(id, Instant::now())
    }

    fn push_at(&self, request: ShareOpenRequest, now: Instant) -> Result<String, ()> {
        let mut entries = self.entries.lock().map_err(|_| ())?;
        prune_expired(&mut entries, now);
        while entries.len() >= self.capacity {
            entries.pop_front();
        }

        let id = Uuid::new_v4().hyphenated().to_string();
        entries.push_back(Entry {
            id: id.clone(),
            request,
            expires_at: now + self.ttl,
        });
        Ok(id)
    }

    fn list_at(&self, now: Instant) -> Result<Vec<String>, ()> {
        let mut entries = self.entries.lock().map_err(|_| ())?;
        prune_expired(&mut entries, now);
        Ok(entries.iter().map(|entry| entry.id.clone()).collect())
    }

    fn take_at(&self, id: &str, now: Instant) -> Result<Option<ShareOpenRequest>, ()> {
        let mut entries = self.entries.lock().map_err(|_| ())?;
        prune_expired(&mut entries, now);
        let Some(index) = entries.iter().position(|entry| entry.id == id) else {
            return Ok(None);
        };
        Ok(entries.remove(index).map(|entry| entry.request))
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

fn is_canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value)
        .ok()
        .is_some_and(|id| !id.is_nil() && id.hyphenated().to_string() == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: &str) -> ShareOpenRequest {
        ShareOpenRequest::Account {
            share_id: id.to_string(),
        }
    }

    #[test]
    fn consumes_entries_once() {
        let state = PendingShareOpenState::for_test(2, Duration::from_secs(60));
        let now = Instant::now();
        let id = state.push_at(request("share-a"), now).unwrap();

        assert_eq!(state.list_at(now).unwrap(), vec![id.clone()]);
        assert!(matches!(
            state.take_at(&id, now).unwrap(),
            Some(ShareOpenRequest::Account { share_id }) if share_id == "share-a"
        ));
        assert!(state.take_at(&id, now).unwrap().is_none());
    }

    #[test]
    fn evicts_the_oldest_entry_at_capacity() {
        let state = PendingShareOpenState::for_test(2, Duration::from_secs(60));
        let now = Instant::now();
        let first = state.push_at(request("share-a"), now).unwrap();
        let second = state.push_at(request("share-b"), now).unwrap();
        let third = state.push_at(request("share-c"), now).unwrap();

        assert_eq!(state.list_at(now).unwrap(), vec![second, third]);
        assert!(state.take_at(&first, now).unwrap().is_none());
    }

    #[test]
    fn drops_expired_entries() {
        let state = PendingShareOpenState::for_test(2, Duration::from_secs(5));
        let now = Instant::now();
        let id = state.push_at(request("share-a"), now).unwrap();

        assert!(
            state
                .list_at(now + Duration::from_secs(5))
                .unwrap()
                .is_empty()
        );
        assert!(
            state
                .take_at(&id, now + Duration::from_secs(5))
                .unwrap()
                .is_none()
        );
    }
}
