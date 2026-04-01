use std::collections::HashMap;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::{DetectEvent, ProcessorState, env::Env, timer_registry::TimerRegistry};

pub(crate) const DEFAULT_MIC_ACTIVE_THRESHOLD_SECS: u64 = 15;
pub(crate) const COOLDOWN_DURATION: Duration = Duration::from_mins(10);

#[derive(Default)]
pub struct MicUsageTracker {
    timers: TimerRegistry,
    cooldowns: HashMap<String, tokio::time::Instant>,
}

impl MicUsageTracker {
    pub fn is_tracking(&self, app_id: &str) -> bool {
        self.timers.contains(app_id)
    }

    pub fn is_in_cooldown(&mut self, app_id: &str) -> bool {
        match self.cooldowns.get(app_id) {
            Some(&fired_at) => {
                if tokio::time::Instant::now().duration_since(fired_at) < COOLDOWN_DURATION {
                    true
                } else {
                    self.cooldowns.remove(app_id);
                    false
                }
            }
            None => false,
        }
    }

    pub fn start_tracking(&mut self, app_id: String, token: CancellationToken) -> u64 {
        self.timers.start_replace(app_id, token)
    }

    pub fn cancel_app(&mut self, app_id: &str) {
        if self.timers.cancel(app_id) {
            tracing::info!(app_id = %app_id, "cancelled_mic_active_timer");
        }
    }

    /// Removes the timer entry only if the generation matches,
    /// preventing a stale timer from claiming an entry replaced by a newer one.
    /// On success, sets a cooldown so the same app won't be re-tracked for a while.
    pub fn claim(&mut self, app_id: &str, generation: u64) -> bool {
        if self.timers.claim(app_id, generation) {
            self.cooldowns
                .insert(app_id.to_string(), tokio::time::Instant::now());
            true
        } else {
            false
        }
    }
}

pub(crate) fn spawn_timer<E: Env>(
    env: E,
    state: ProcessorState,
    app: hypr_detect::InstalledApp,
    generation: u64,
    token: CancellationToken,
    threshold_secs: u64,
) {
    let duration = Duration::from_secs(threshold_secs);
    let app_id = app.id.clone();

    tokio::spawn(async move {
        tokio::select! {
            _ = tokio::time::sleep(duration) => {}
            _ = token.cancelled() => { return; }
        }

        let emit_event = {
            let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
            if !guard.mic_usage_tracker.claim(&app_id, generation) {
                None
            } else if !env.is_detect_enabled() {
                tracing::info!(app_id = %app_id, "skip_mic_detected: detect_disabled");
                None
            } else if guard.policy.respect_dnd && env.is_do_not_disturb() {
                tracing::info!(app_id = %app_id, "skip_mic_detected: DoNotDisturb");
                None
            } else {
                let key = uuid::Uuid::new_v4().to_string();
                Some(DetectEvent::MicDetected {
                    key,
                    apps: vec![app.clone()],
                    duration_secs: threshold_secs,
                })
            }
        };

        if let Some(event) = emit_event {
            tracing::info!(app_id = %app.id, threshold_secs, "mic_detected");
            env.emit(event);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn test_cooldown_blocks_retracking() {
        let mut tracker = MicUsageTracker::default();

        let generation = tracker.start_tracking("app.x".to_string(), CancellationToken::new());
        assert!(tracker.claim("app.x", generation));
        assert!(tracker.is_in_cooldown("app.x"));

        tokio::time::advance(Duration::from_secs(5 * 60)).await;
        assert!(
            tracker.is_in_cooldown("app.x"),
            "still in cooldown at 5 min"
        );

        tokio::time::advance(Duration::from_secs(5 * 60)).await;
        assert!(
            !tracker.is_in_cooldown("app.x"),
            "cooldown expired at 10 min"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_no_cooldown_without_claim() {
        let mut tracker = MicUsageTracker::default();
        tracker.start_tracking("app.x".to_string(), CancellationToken::new());
        tracker.cancel_app("app.x");
        assert!(!tracker.is_in_cooldown("app.x"));
    }

    #[test]
    fn test_cancel_nonexistent_app_is_noop() {
        let mut tracker = MicUsageTracker::default();
        tracker.cancel_app("app.nonexistent");
    }

    #[test]
    fn test_claim_nonexistent_app_returns_false() {
        let _rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = _rt.enter();

        let mut tracker = MicUsageTracker::default();
        assert!(!tracker.claim("app.nonexistent", 0));
    }

    #[test]
    fn test_is_tracking_reflects_state() {
        let mut tracker = MicUsageTracker::default();

        assert!(!tracker.is_tracking("app.x"));

        tracker.start_tracking("app.x".to_string(), CancellationToken::new());
        assert!(tracker.is_tracking("app.x"));

        tracker.cancel_app("app.x");
        assert!(!tracker.is_tracking("app.x"));
    }

    #[test]
    fn test_drop_cancels_all_tokens() {
        let token1 = CancellationToken::new();
        let token2 = CancellationToken::new();
        let t1_clone = token1.clone();
        let t2_clone = token2.clone();

        {
            let mut tracker = MicUsageTracker::default();
            tracker.start_tracking("app.a".to_string(), token1);
            tracker.start_tracking("app.b".to_string(), token2);
            assert!(!t1_clone.is_cancelled());
            assert!(!t2_clone.is_cancelled());
        }

        assert!(
            t1_clone.is_cancelled(),
            "token1 should be cancelled on drop"
        );
        assert!(
            t2_clone.is_cancelled(),
            "token2 should be cancelled on drop"
        );
    }

    #[test]
    fn test_generation_increments() {
        let mut tracker = MicUsageTracker::default();
        let g0 = tracker.start_tracking("app.a".to_string(), CancellationToken::new());
        let g1 = tracker.start_tracking("app.b".to_string(), CancellationToken::new());
        let g2 = tracker.start_tracking("app.c".to_string(), CancellationToken::new());
        assert_eq!(g0, 0);
        assert_eq!(g1, 1);
        assert_eq!(g2, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn test_claim_sets_cooldown() {
        let mut tracker = MicUsageTracker::default();
        let generation = tracker.start_tracking("app.x".to_string(), CancellationToken::new());

        assert!(!tracker.is_in_cooldown("app.x"), "no cooldown before claim");
        assert!(tracker.claim("app.x", generation));
        assert!(tracker.is_in_cooldown("app.x"), "cooldown set after claim");
    }

    #[tokio::test(start_paused = true)]
    async fn test_independent_cooldowns() {
        let mut tracker = MicUsageTracker::default();

        let g_a = tracker.start_tracking("app.a".to_string(), CancellationToken::new());
        assert!(tracker.claim("app.a", g_a));

        tokio::time::advance(Duration::from_secs(5 * 60)).await;

        let g_b = tracker.start_tracking("app.b".to_string(), CancellationToken::new());
        assert!(tracker.claim("app.b", g_b));

        assert!(
            tracker.is_in_cooldown("app.a"),
            "app.a still in cooldown at 5 min"
        );
        assert!(
            tracker.is_in_cooldown("app.b"),
            "app.b just started cooldown"
        );

        tokio::time::advance(Duration::from_secs(5 * 60)).await;

        assert!(
            !tracker.is_in_cooldown("app.a"),
            "app.a cooldown expired at 10 min"
        );
        assert!(
            tracker.is_in_cooldown("app.b"),
            "app.b still in cooldown at 5 min"
        );
    }

    #[test]
    fn test_cancel_app_twice_is_safe() {
        let mut tracker = MicUsageTracker::default();
        tracker.start_tracking("app.x".to_string(), CancellationToken::new());
        tracker.cancel_app("app.x");
        tracker.cancel_app("app.x");
        assert!(!tracker.is_tracking("app.x"));
    }
}
