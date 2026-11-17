//! Shipping's `DictationController` (`dictation/controller.ts`) as a synchronous state
//! machine. Each input returns the work the host must start; the host reports completion back
//! (`started`, `released`, `finished`), so press/release/cancel ordering stays testable without
//! audio, providers or windows.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    Starting,
    Recording,
    Transcribing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Check permissions, capture the focused field and open the microphone.
    Start,
    /// Stop the recording, transcribe it and insert the text.
    Finish,
    /// Release the microphone without transcribing.
    Cancel,
}

/// Recordings end on their own after five minutes, like shipping's `setTimeout`.
pub const MAX_RECORDING: std::time::Duration = std::time::Duration::from_secs(300);

#[derive(Debug, Default)]
pub struct Controller {
    phase: Phase,
    hands_free: bool,
    cancelled: bool,
    released: bool,
    /// Identifies the current recording so a stale five-minute timer cannot finish a newer one.
    recording: u64,
}

impl Controller {
    pub fn new(hands_free: bool) -> Self {
        Self {
            hands_free,
            ..Self::default()
        }
    }

    /// Applies the hands-free setting when dictation is armed again.
    pub fn set_hands_free(&mut self, hands_free: bool) {
        self.hands_free = hands_free;
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Whether the user cancelled; late results and errors are then dropped.
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn recording(&self) -> u64 {
        self.recording
    }

    pub fn press(&mut self) -> Option<Effect> {
        if self.phase == Phase::Idle {
            self.cancelled = false;
            self.released = false;
            self.phase = Phase::Starting;
            Some(Effect::Start)
        } else if self.hands_free && self.phase != Phase::Transcribing {
            self.release(true)
        } else {
            None
        }
    }

    /// The shortcut's release. Hands-free dictation ignores it; `force` is the second press,
    /// the panel's Done button or the time limit.
    pub fn release(&mut self, force: bool) -> Option<Effect> {
        if self.hands_free && !force {
            return None;
        }
        match self.phase {
            Phase::Starting => {
                self.released = true;
                None
            }
            Phase::Recording => {
                self.phase = Phase::Transcribing;
                Some(Effect::Finish)
            }
            _ => None,
        }
    }

    /// The five-minute limit for `recording`.
    pub fn timeout(&mut self, recording: u64) -> Option<Effect> {
        (recording == self.recording)
            .then(|| self.release(true))
            .flatten()
    }

    pub fn cancel(&mut self) -> Option<Effect> {
        self.cancelled = true;
        if self.phase == Phase::Recording {
            // Stay busy until this recording has relinquished the microphone.
            self.phase = Phase::Starting;
            return Some(Effect::Cancel);
        }
        None
    }

    /// The start work finished; `ok` means the microphone is now recording.
    pub fn started(&mut self, ok: bool) -> Option<Effect> {
        if self.phase != Phase::Starting {
            return None;
        }
        if !ok {
            // A rejected start never owns the recorder (chat may already be using it).
            self.phase = Phase::Idle;
            return None;
        }
        if self.cancelled {
            return Some(Effect::Cancel);
        }
        self.phase = Phase::Recording;
        self.recording += 1;
        if self.released {
            self.phase = Phase::Transcribing;
            return Some(Effect::Finish);
        }
        None
    }

    /// The microphone was released after `Effect::Cancel`.
    pub fn released(&mut self) {
        self.phase = Phase::Idle;
    }

    /// The finish work ended, inserted or not.
    pub fn finished(&mut self) {
        self.phase = Phase::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_to_talk_records_until_release_and_finishes_once() {
        let mut controller = Controller::new(false);
        assert_eq!(controller.press(), Some(Effect::Start));
        assert_eq!(controller.press(), None);
        assert_eq!(controller.started(true), None);
        assert_eq!(controller.phase(), Phase::Recording);
        assert_eq!(controller.release(false), Some(Effect::Finish));
        assert_eq!(controller.phase(), Phase::Transcribing);
        assert_eq!(controller.release(false), None);
        controller.finished();
        assert_eq!(controller.phase(), Phase::Idle);
    }

    #[test]
    fn releasing_while_the_microphone_opens_finishes_as_soon_as_it_records() {
        let mut controller = Controller::new(false);
        controller.press();
        assert_eq!(controller.release(false), None);
        assert_eq!(controller.started(true), Some(Effect::Finish));
        assert_eq!(controller.phase(), Phase::Transcribing);
    }

    #[test]
    fn hands_free_ignores_release_and_finishes_on_the_second_press() {
        let mut controller = Controller::new(true);
        controller.press();
        controller.started(true);
        assert_eq!(controller.release(false), None);
        assert_eq!(controller.press(), Some(Effect::Finish));
        // Pressing while transcribing does nothing; the next dictation starts after it ends.
        assert_eq!(controller.press(), None);
        controller.finished();
        assert_eq!(controller.press(), Some(Effect::Start));
    }

    #[test]
    fn cancel_releases_the_microphone_before_returning_to_idle() {
        let mut controller = Controller::new(false);
        controller.press();
        controller.started(true);
        assert_eq!(controller.cancel(), Some(Effect::Cancel));
        assert_eq!(controller.phase(), Phase::Starting);
        assert_eq!(controller.press(), None);
        controller.released();
        assert_eq!(controller.phase(), Phase::Idle);
        assert_eq!(controller.press(), Some(Effect::Start));
        assert!(!controller.cancelled());
    }

    #[test]
    fn cancel_while_starting_releases_the_microphone_once_it_opens() {
        let mut controller = Controller::new(false);
        controller.press();
        assert_eq!(controller.cancel(), None);
        assert_eq!(controller.started(true), Some(Effect::Cancel));
        controller.released();
        assert_eq!(controller.phase(), Phase::Idle);
    }

    #[test]
    fn a_failed_start_returns_to_idle_without_owning_the_recorder() {
        let mut controller = Controller::new(false);
        controller.press();
        controller.release(false);
        assert_eq!(controller.started(false), None);
        assert_eq!(controller.phase(), Phase::Idle);
    }

    #[test]
    fn only_the_current_recordings_time_limit_finishes_it() {
        let mut controller = Controller::new(false);
        controller.press();
        controller.started(true);
        let first = controller.recording();
        controller.release(false);
        controller.finished();
        controller.press();
        controller.started(true);
        assert_eq!(controller.timeout(first), None);
        assert_eq!(
            controller.timeout(controller.recording()),
            Some(Effect::Finish)
        );
    }
}
