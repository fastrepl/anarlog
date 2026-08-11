use std::ptr::NonNull;
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_foundation::{MainThreadMarker, NSTimer};

/// A repeating main-run-loop timer that retains its callback for the timer's
/// lifetime and invalidates defensively on drop.
pub struct RepeatingTimer {
    timer: Retained<NSTimer>,
}

impl RepeatingTimer {
    pub fn schedule(
        mtm: MainThreadMarker,
        interval: Duration,
        callback: impl Fn() + 'static,
    ) -> Self {
        let _ = mtm;
        let seconds = interval.as_secs_f64();
        assert!(
            seconds.is_finite() && seconds > 0.0,
            "repeating timer interval must be finite and positive"
        );
        let block = RcBlock::new(move |_timer: NonNull<NSTimer>| callback());
        let timer =
            unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(seconds, true, &block) };
        Self { timer }
    }

    pub fn invalidate(&self) {
        self.timer.invalidate();
    }
}

impl Drop for RepeatingTimer {
    fn drop(&mut self) {
        self.timer.invalidate();
    }
}

impl std::fmt::Debug for RepeatingTimer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepeatingTimer").finish_non_exhaustive()
    }
}
