use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RestartBudget {
    pub max_restarts: u32,
    pub max_window: Duration,
    pub reset_after: Option<Duration>,
}

impl Default for RestartBudget {
    fn default() -> Self {
        Self {
            max_restarts: 3,
            max_window: Duration::from_secs(15),
            reset_after: Some(Duration::from_secs(30)),
        }
    }
}

pub struct RestartTracker {
    count: u32,
    window_start: Instant,
}

impl Default for RestartTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl RestartTracker {
    pub fn new() -> Self {
        Self {
            count: 0,
            window_start: Instant::now(),
        }
    }

    /// Records a restart and returns whether the budget still allows it.
    /// `true` = within budget, `false` = meltdown threshold exceeded.
    pub fn record_restart(&mut self, budget: &RestartBudget) -> bool {
        let now = Instant::now();
        if now.duration_since(self.window_start) > budget.max_window {
            self.count = 0;
            self.window_start = now;
        }
        self.count += 1;
        self.count <= budget.max_restarts
    }

    /// Resets the counter if quiet for longer than `budget.reset_after`.
    pub fn maybe_reset(&mut self, budget: &RestartBudget) {
        if let Some(reset_after) = budget.reset_after {
            let now = Instant::now();
            if now.duration_since(self.window_start) > reset_after {
                self.count = 0;
                self.window_start = now;
            }
        }
    }

    pub fn count(&self) -> u32 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget(max_restarts: u32, max_window_ms: u64) -> RestartBudget {
        RestartBudget {
            max_restarts,
            max_window: Duration::from_millis(max_window_ms),
            reset_after: None,
        }
    }

    #[test]
    fn new_tracker_starts_at_zero() {
        let tracker = RestartTracker::new();
        assert_eq!(tracker.count(), 0);
    }

    #[test]
    fn record_restart_increments_within_budget() {
        let mut tracker = RestartTracker::new();
        let b = budget(3, 10_000);
        assert!(tracker.record_restart(&b));
        assert!(tracker.record_restart(&b));
        assert!(tracker.record_restart(&b));
        assert_eq!(tracker.count(), 3);
    }

    #[test]
    fn record_restart_exceeds_budget() {
        let mut tracker = RestartTracker::new();
        let b = budget(2, 10_000);
        assert!(tracker.record_restart(&b));
        assert!(tracker.record_restart(&b));
        assert!(!tracker.record_restart(&b));
    }

    #[test]
    fn record_restart_at_exact_boundary() {
        let mut tracker = RestartTracker::new();
        let b = budget(1, 10_000);
        assert!(tracker.record_restart(&b));
        assert!(!tracker.record_restart(&b));
    }

    #[test]
    fn window_expiry_resets_counter() {
        let mut tracker = RestartTracker::new();
        let b = budget(1, 50);
        assert!(tracker.record_restart(&b));
        std::thread::sleep(Duration::from_millis(100));
        assert!(tracker.record_restart(&b));
    }

    #[test]
    fn maybe_reset_clears_after_quiet_period() {
        let mut tracker = RestartTracker::new();
        let mut b = budget(3, 10_000);
        b.reset_after = Some(Duration::from_millis(50));

        tracker.record_restart(&b);
        tracker.record_restart(&b);
        assert_eq!(tracker.count(), 2);

        std::thread::sleep(Duration::from_millis(100));
        tracker.maybe_reset(&b);
        assert_eq!(tracker.count(), 0);
    }

    #[test]
    fn maybe_reset_noop_before_threshold() {
        let mut tracker = RestartTracker::new();
        let mut b = budget(3, 10_000);
        b.reset_after = Some(Duration::from_secs(60));

        tracker.record_restart(&b);
        tracker.record_restart(&b);
        tracker.maybe_reset(&b);
        assert_eq!(tracker.count(), 2);
    }

    #[test]
    fn maybe_reset_noop_when_none() {
        let mut tracker = RestartTracker::new();
        let b = budget(3, 10_000);

        tracker.record_restart(&b);
        tracker.maybe_reset(&b);
        assert_eq!(tracker.count(), 1);
    }

    #[test]
    fn zero_budget_always_exceeds() {
        let mut tracker = RestartTracker::new();
        let b = budget(0, 10_000);
        assert!(!tracker.record_restart(&b));
    }
}
