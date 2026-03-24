use std::time::Duration;

use crate::InstalledApp;

pub(super) const START_CONFIRMATION: Duration = Duration::from_millis(500);
pub(super) const STOP_GRACE: Duration = Duration::from_secs(3);

pub(super) struct DetectorState {
    pub(super) last_state: bool,
    pending_state: Option<bool>,
    start_confirmation: Duration,
    stop_grace: Duration,
    transition_generation: u64,
    pub(super) active_apps: Vec<InstalledApp>,
}

#[derive(Clone, Copy)]
pub(super) struct PendingTransition {
    pub(super) expected_state: bool,
    pub(super) generation: u64,
    pub(super) delay: Duration,
}

pub(super) struct ObserveOutcome {
    pub(super) pending: Option<PendingTransition>,
    pub(super) cancelled_stop: bool,
}

impl DetectorState {
    pub(super) fn new() -> Self {
        Self {
            last_state: false,
            pending_state: None,
            start_confirmation: START_CONFIRMATION,
            stop_grace: STOP_GRACE,
            transition_generation: 0,
            active_apps: Vec::new(),
        }
    }

    pub(super) fn observe(&mut self, new_state: bool) -> ObserveOutcome {
        let cancelled_stop = self.pending_state == Some(false) && new_state;

        self.transition_generation += 1;
        self.pending_state = None;

        if new_state == self.last_state {
            return ObserveOutcome {
                pending: None,
                cancelled_stop,
            };
        }

        let delay = if new_state {
            self.start_confirmation
        } else {
            self.stop_grace
        };
        self.pending_state = Some(new_state);

        ObserveOutcome {
            pending: Some(PendingTransition {
                expected_state: new_state,
                generation: self.transition_generation,
                delay,
            }),
            cancelled_stop,
        }
    }

    pub(super) fn cancel_pending(&mut self) -> bool {
        let cancelled_stop = self.pending_state == Some(false);
        self.transition_generation += 1;
        self.pending_state = None;
        cancelled_stop
    }

    pub(super) fn commit(&mut self, confirmed_state: bool, generation: u64) -> bool {
        if generation != self.transition_generation
            || self.pending_state != Some(confirmed_state)
            || confirmed_state == self.last_state
        {
            return false;
        }

        self.last_state = confirmed_state;
        self.pending_state = None;
        true
    }
}
