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
    fn keeps_the_session_while_activity_is_recent() {
        let now = SystemTime::UNIX_EPOCH;
        let mut tracker = SessionTracker::new(now);
        let first = tracker.session_id(now);
        let next = tracker.session_id(now + IDLE_TIMEOUT - Duration::from_millis(1));

        assert_eq!(next, first);
    }

    #[test]
    fn rotates_the_session() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let activity_interval = IDLE_TIMEOUT - Duration::from_secs(1);
        let mut continuous_activity = Vec::new();
        let mut elapsed = activity_interval;
        loop {
            continuous_activity.push((now + elapsed, elapsed >= MAX_DURATION));
            if elapsed >= MAX_DURATION {
                break;
            }
            elapsed += activity_interval;
        }

        let cases = [
            ("idle timeout", vec![(now + IDLE_TIMEOUT, true)]),
            ("continuous activity", continuous_activity),
            (
                "wall clock moving backwards",
                vec![(SystemTime::UNIX_EPOCH, true)],
            ),
        ];

        for (case, times) in cases {
            let mut tracker = SessionTracker::new(now);
            let first = tracker.session_id(now);
            for (at, should_rotate) in times {
                assert_eq!(
                    tracker.session_id(at) != first,
                    should_rotate,
                    "{case} at {at:?}"
                );
            }
        }
    }
}
