use std::time::{Duration, SystemTime};

const IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const MAX_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

pub(crate) struct SessionTracker {
    id: uuid::Uuid,
    started_at: SystemTime,
    last_activity_at: SystemTime,
}

impl SessionTracker {
    pub(crate) fn new(now: SystemTime) -> Self {
        Self {
            id: uuid::Uuid::now_v7(),
            started_at: now,
            last_activity_at: now,
        }
    }

    pub(crate) fn session_id(&mut self, now: SystemTime) -> String {
        let idle_timed_out = now
            .duration_since(self.last_activity_at)
            .is_ok_and(|elapsed| elapsed >= IDLE_TIMEOUT);
        let max_duration_reached = now
            .duration_since(self.started_at)
            .is_ok_and(|elapsed| elapsed >= MAX_DURATION);

        if now < self.started_at
            || now < self.last_activity_at
            || idle_timed_out
            || max_duration_reached
        {
            *self = Self::new(now);
        } else {
            self.last_activity_at = now;
        }

        self.id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_uuid_v7_session() {
        let now = SystemTime::UNIX_EPOCH;
        let mut tracker = SessionTracker::new(now);
        let id = uuid::Uuid::parse_str(&tracker.session_id(now)).unwrap();

        assert_eq!(id.get_version_num(), 7);
    }

    #[test]
    fn keeps_the_session_while_activity_is_recent() {
        let now = SystemTime::UNIX_EPOCH;
        let mut tracker = SessionTracker::new(now);
        let first = tracker.session_id(now);
        let next = tracker.session_id(now + IDLE_TIMEOUT - Duration::from_millis(1));

        assert_eq!(next, first);
    }

    #[test]
    fn rotates_after_the_idle_timeout() {
        let now = SystemTime::UNIX_EPOCH;
        let mut tracker = SessionTracker::new(now);
        let first = tracker.session_id(now);
        let next = tracker.session_id(now + IDLE_TIMEOUT);

        assert_ne!(next, first);
    }

    #[test]
    fn rotates_at_the_maximum_session_duration() {
        let now = SystemTime::UNIX_EPOCH;
        let mut tracker = SessionTracker::new(now);
        let first = tracker.session_id(now);
        tracker.last_activity_at = now + MAX_DURATION - Duration::from_millis(1);
        let next = tracker.session_id(now + MAX_DURATION);

        assert_ne!(next, first);
    }

    #[test]
    fn rotates_when_the_wall_clock_moves_backwards() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let mut tracker = SessionTracker::new(now);
        let first = tracker.session_id(now);
        let next = tracker.session_id(SystemTime::UNIX_EPOCH);

        assert_ne!(next, first);
    }
}
