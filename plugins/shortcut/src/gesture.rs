//! Hold / latch / cancel gestures for the dictation shortcut, shared by every backend.
//!
//! Behaviour follows Char's voice hotkey controller (`desktop-dictation/src/main/everywhere/
//! hotkey.ts`, reference only): a press primes, becomes a dictation once it outlasts a tap,
//! and stops on release. A second tap soon after a tap, or the latch key during a press,
//! keeps listening until the next press. Escape, or another key joining a press, cancels.
//! A tap never starts a dictation.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GestureOptions {
    #[serde(default = "default_hold_ms")]
    pub hold_ms: u64,
    #[serde(default = "default_double_tap_ms")]
    pub double_tap_ms: u64,
    #[serde(default = "default_true")]
    pub double_tap_latch: bool,
    #[serde(default = "default_true")]
    pub space_latch: bool,
}

impl Default for GestureOptions {
    fn default() -> Self {
        Self {
            hold_ms: default_hold_ms(),
            double_tap_ms: default_double_tap_ms(),
            double_tap_latch: true,
            space_latch: true,
        }
    }
}

fn default_hold_ms() -> u64 {
    250
}

fn default_double_tap_ms() -> u64 {
    400
}

fn default_true() -> bool {
    true
}

/// What the backend can observe, so settings can say which gestures work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GestureCapabilities {
    pub space_latch: bool,
    pub chord_cancel: bool,
}

crate::common_event_derives! {
    #[derive(Debug, PartialEq, Eq)]
    #[serde(tag = "type", rename_all = "camelCase")]
    pub enum DictationGesture {
        /// A press began and may become a dictation. Prepare without prompting for anything.
        Prime,
        /// The primed press ended as a tap or a chord: drop what it prepared.
        Unprime,
        /// Dictation starts. A latched start after a double tap was not primed.
        Start { latched: bool },
        /// The running hold keeps listening after release.
        Latch,
        Stop,
        Cancel,
        /// A single tap with no second tap after it.
        Tap,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Pending { generation: u64 },
    Holding,
    Latched,
}

/// Events to emit, plus timers the driver must schedule.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Step {
    pub events: Vec<DictationGesture>,
    pub hold_timer: Option<(u64, Duration)>,
    pub tap_timer: Option<(u64, Duration)>,
}

impl Step {
    fn event(event: DictationGesture) -> Self {
        Self {
            events: vec![event],
            ..Self::default()
        }
    }
}

pub struct Gesture {
    options: GestureOptions,
    state: State,
    generation: u64,
    last_tap: Option<Instant>,
    tap_pending: Option<u64>,
}

impl Gesture {
    pub fn new(options: GestureOptions) -> Self {
        Self {
            options,
            state: State::Idle,
            generation: 0,
            last_tap: None,
            tap_pending: None,
        }
    }

    pub fn down(&mut self, now: Instant) -> Step {
        match self.state {
            State::Pending { .. } | State::Holding => Step::default(),
            State::Latched => {
                self.state = State::Idle;
                self.last_tap = None;
                Step::event(DictationGesture::Stop)
            }
            State::Idle => {
                self.tap_pending = None;
                let double_tap = self.options.double_tap_latch
                    && self.last_tap.is_some_and(|tap| {
                        now.saturating_duration_since(tap)
                            < Duration::from_millis(self.options.double_tap_ms)
                    });
                self.last_tap = None;
                if double_tap {
                    self.state = State::Latched;
                    return Step::event(DictationGesture::Start { latched: true });
                }
                self.generation += 1;
                self.state = State::Pending {
                    generation: self.generation,
                };
                Step {
                    events: vec![DictationGesture::Prime],
                    hold_timer: Some((
                        self.generation,
                        Duration::from_millis(self.options.hold_ms),
                    )),
                    tap_timer: None,
                }
            }
        }
    }

    pub fn up(&mut self, now: Instant) -> Step {
        match self.state {
            State::Pending { .. } => {
                self.state = State::Idle;
                self.generation += 1;
                self.last_tap = Some(now);
                self.tap_pending = Some(self.generation);
                Step {
                    events: vec![DictationGesture::Unprime],
                    hold_timer: None,
                    tap_timer: Some((
                        self.generation,
                        Duration::from_millis(self.options.double_tap_ms),
                    )),
                }
            }
            State::Holding => {
                self.state = State::Idle;
                Step::event(DictationGesture::Stop)
            }
            State::Idle | State::Latched => Step::default(),
        }
    }

    pub fn hold_elapsed(&mut self, generation: u64) -> Step {
        if self.state != (State::Pending { generation }) {
            return Step::default();
        }
        self.state = State::Holding;
        Step::event(DictationGesture::Start { latched: false })
    }

    pub fn tap_elapsed(&mut self, generation: u64) -> Step {
        if self.tap_pending != Some(generation) || self.state != State::Idle {
            return Step::default();
        }
        self.tap_pending = None;
        Step::event(DictationGesture::Tap)
    }

    /// The latch key (Space) while the shortcut is held. Empty when it did nothing, so the
    /// backend lets the key through.
    pub fn latch_key(&mut self) -> Step {
        if !self.options.space_latch {
            return Step::default();
        }
        match self.state {
            State::Pending { .. } => {
                self.state = State::Latched;
                Step::event(DictationGesture::Start { latched: true })
            }
            State::Holding => {
                self.state = State::Latched;
                Step::event(DictationGesture::Latch)
            }
            State::Idle | State::Latched => Step::default(),
        }
    }

    /// Another key or modifier joined the held shortcut: it was a chord, not dictation.
    pub fn chord(&mut self) -> Step {
        self.forget_taps();
        match self.state {
            State::Pending { .. } => {
                self.state = State::Idle;
                Step::event(DictationGesture::Unprime)
            }
            State::Holding => {
                self.state = State::Idle;
                Step::event(DictationGesture::Cancel)
            }
            State::Idle | State::Latched => Step::default(),
        }
    }

    /// A mouse click during a modifier-only press that has not started (Command-click).
    pub fn click(&mut self) -> Step {
        if !matches!(self.state, State::Pending { .. }) {
            return Step::default();
        }
        self.forget_taps();
        self.state = State::Idle;
        Step::event(DictationGesture::Unprime)
    }

    /// Escape. `dictation_active`: the app still shows a dictation, perhaps one finishing.
    pub fn escape(&mut self, dictation_active: bool) -> Step {
        self.forget_taps();
        let state = std::mem::replace(&mut self.state, State::Idle);
        match state {
            State::Pending { .. } => Step {
                events: vec![DictationGesture::Unprime, DictationGesture::Cancel],
                ..Step::default()
            },
            State::Holding | State::Latched => Step::event(DictationGesture::Cancel),
            State::Idle if dictation_active => Step::event(DictationGesture::Cancel),
            State::Idle => Step::default(),
        }
    }

    /// The backend stopped delivering events, so no release will come.
    pub fn lost(&mut self) -> Step {
        self.forget_taps();
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Pending { .. } => Step::event(DictationGesture::Unprime),
            State::Holding | State::Latched => Step::event(DictationGesture::Cancel),
            State::Idle => Step::default(),
        }
    }

    /// The dictation ended on its own (limit, error, Finish/Cancel in the UI). A press still
    /// waiting out its hold is the next dictation, so it is kept.
    pub fn session_ended(&mut self) {
        if matches!(self.state, State::Holding | State::Latched) {
            self.state = State::Idle;
        }
    }

    fn forget_taps(&mut self) {
        self.last_tap = None;
        self.tap_pending = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Down,
    Up,
    // Only the macOS event tap sees other keys and clicks during a press.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    LatchKey,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Chord,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Click,
    Escape,
    // Only the Wayland portal can lose its session.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Lost,
}

type Emit = Box<dyn Fn(DictationGesture) + Send + Sync>;

/// Feeds backend input into one gesture machine and runs its timers.
pub struct Driver {
    machine: Mutex<Gesture>,
    emit: Emit,
    active: Arc<AtomicBool>,
    capabilities: GestureCapabilities,
}

impl Driver {
    pub fn new(
        options: GestureOptions,
        capabilities: GestureCapabilities,
        active: Arc<AtomicBool>,
        emit: impl Fn(DictationGesture) + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            machine: Mutex::new(Gesture::new(options)),
            emit: Box::new(emit),
            active,
            capabilities,
        })
    }

    pub fn capabilities(&self) -> GestureCapabilities {
        self.capabilities
    }

    /// Returns whether the input did something, so a backend may swallow the key.
    pub fn input(self: &Arc<Self>, input: Input) -> bool {
        self.handle(Event::Input(input))
    }

    pub fn session_ended(&self) {
        self.machine
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .session_ended();
    }

    fn handle(self: &Arc<Self>, event: Event) -> bool {
        let now = Instant::now();
        // Emitting under the lock keeps events from different backend threads in order.
        let mut machine = self
            .machine
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let step = match event {
            Event::Input(Input::Down) => machine.down(now),
            Event::Input(Input::Up) => machine.up(now),
            Event::Input(Input::LatchKey) => machine.latch_key(),
            Event::Input(Input::Chord) => machine.chord(),
            Event::Input(Input::Click) => machine.click(),
            Event::Input(Input::Escape) => machine.escape(self.active.load(Ordering::SeqCst)),
            Event::Input(Input::Lost) => machine.lost(),
            Event::HoldElapsed(generation) => machine.hold_elapsed(generation),
            Event::TapElapsed(generation) => machine.tap_elapsed(generation),
        };
        for event in &step.events {
            (self.emit)(event.clone());
        }
        drop(machine);
        if let Some((generation, delay)) = step.hold_timer {
            self.after(delay, Event::HoldElapsed(generation));
        }
        if let Some((generation, delay)) = step.tap_timer {
            self.after(delay, Event::TapElapsed(generation));
        }
        !step.events.is_empty()
    }

    fn after(self: &Arc<Self>, delay: Duration, event: Event) {
        let driver = Arc::downgrade(self);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(delay).await;
            if let Some(driver) = driver.upgrade() {
                driver.handle(event);
            }
        });
    }
}

#[derive(Debug, Clone, Copy)]
enum Event {
    Input(Input),
    HoldElapsed(u64),
    TapElapsed(u64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use DictationGesture::*;

    struct Run {
        gesture: Gesture,
        start: Instant,
        events: Vec<DictationGesture>,
        hold: Option<u64>,
        tap: Option<u64>,
    }

    impl Run {
        fn new() -> Self {
            Self::with(GestureOptions::default())
        }

        fn with(options: GestureOptions) -> Self {
            Self {
                gesture: Gesture::new(options),
                start: Instant::now(),
                events: Vec::new(),
                hold: None,
                tap: None,
            }
        }

        fn at(&self, ms: u64) -> Instant {
            self.start + Duration::from_millis(ms)
        }

        fn take(&mut self, step: Step) {
            if let Some((generation, _)) = step.hold_timer {
                self.hold = Some(generation);
            }
            if let Some((generation, _)) = step.tap_timer {
                self.tap = Some(generation);
            }
            self.events.extend(step.events);
        }

        fn down(&mut self, ms: u64) {
            let step = self.gesture.down(self.at(ms));
            self.take(step);
        }

        fn up(&mut self, ms: u64) {
            let step = self.gesture.up(self.at(ms));
            self.take(step);
        }

        fn hold_elapses(&mut self) {
            let step = self.gesture.hold_elapsed(self.hold.take().unwrap());
            self.take(step);
        }

        fn tap_elapses(&mut self) {
            let step = self.gesture.tap_elapsed(self.tap.take().unwrap());
            self.take(step);
        }

        fn drain(&mut self) -> Vec<DictationGesture> {
            std::mem::take(&mut self.events)
        }
    }

    #[test]
    fn hold_starts_after_the_threshold_and_stops_on_release() {
        let mut run = Run::new();
        run.down(0);
        assert_eq!(run.drain(), [Prime]);
        run.hold_elapses();
        run.up(900);
        assert_eq!(run.drain(), [Start { latched: false }, Stop]);
    }

    #[test]
    fn a_tap_never_starts_and_reports_itself_once_no_second_tap_follows() {
        let mut run = Run::new();
        run.down(0);
        run.up(100);
        let stale_hold = run.hold.take().unwrap();
        let step = run.gesture.hold_elapsed(stale_hold);
        run.take(step);
        run.tap_elapses();
        assert_eq!(run.drain(), [Prime, Unprime, Tap]);
    }

    #[test]
    fn double_tap_latches_without_priming_until_the_next_press() {
        let mut run = Run::new();
        run.down(0);
        run.up(80);
        let first_tap = run.tap.take().unwrap();
        run.down(300);
        run.up(380);
        // The first tap's timer fires late: it was a latch, not a tap.
        let step = run.gesture.tap_elapsed(first_tap);
        run.take(step);
        run.down(5_000);
        run.up(5_050);
        assert_eq!(run.drain(), [Prime, Unprime, Start { latched: true }, Stop]);
    }

    #[test]
    fn a_second_tap_after_the_window_is_a_fresh_press() {
        let mut run = Run::new();
        run.down(0);
        run.up(80);
        run.down(600);
        assert_eq!(run.drain(), [Prime, Unprime, Prime]);
    }

    #[test]
    fn latch_key_keeps_a_hold_listening_and_can_latch_a_press_before_the_threshold() {
        let mut run = Run::new();
        run.down(0);
        run.hold_elapses();
        let step = run.gesture.latch_key();
        run.take(step);
        run.up(1_000);
        run.down(3_000);
        run.up(3_050);
        assert_eq!(run.drain(), [Prime, Start { latched: false }, Latch, Stop]);

        let mut early = Run::new();
        early.down(0);
        let step = early.gesture.latch_key();
        early.take(step);
        early.hold_elapses();
        assert_eq!(early.drain(), [Prime, Start { latched: true }]);
    }

    #[test]
    fn latch_key_is_ignored_when_disabled_or_idle() {
        let mut run = Run::with(GestureOptions {
            space_latch: false,
            ..GestureOptions::default()
        });
        assert!(run.gesture.latch_key().events.is_empty());
        run.down(0);
        assert!(run.gesture.latch_key().events.is_empty());
    }

    #[test]
    fn chords_unprime_a_pending_press_and_cancel_a_hold() {
        let mut run = Run::new();
        run.down(0);
        let step = run.gesture.chord();
        run.take(step);
        run.up(100);
        run.down(200);
        run.hold_elapses();
        let step = run.gesture.chord();
        run.take(step);
        run.up(900);
        assert_eq!(
            run.drain(),
            [Prime, Unprime, Prime, Start { latched: false }, Cancel]
        );
    }

    #[test]
    fn a_chord_is_not_a_tap_for_double_tap_latch() {
        let mut run = Run::new();
        run.down(0);
        let step = run.gesture.chord();
        run.take(step);
        run.up(50);
        run.down(150);
        assert_eq!(run.drain(), [Prime, Unprime, Prime]);
    }

    #[test]
    fn escape_cancels_a_press_a_hold_and_a_finishing_dictation() {
        let mut run = Run::new();
        run.down(0);
        let step = run.gesture.escape(false);
        run.take(step);
        assert_eq!(run.drain(), [Prime, Unprime, Cancel]);

        run.down(1_000);
        run.hold_elapses();
        let step = run.gesture.escape(false);
        run.take(step);
        assert_eq!(run.drain(), [Prime, Start { latched: false }, Cancel]);

        assert!(run.gesture.escape(false).events.is_empty());
        assert_eq!(run.gesture.escape(true).events, [Cancel]);
    }

    #[test]
    fn a_dictation_ending_on_its_own_frees_a_latch_but_not_the_next_press() {
        let mut run = Run::new();
        run.down(0);
        run.up(50);
        run.down(200);
        run.gesture.session_ended();
        run.down(3_000);
        assert_eq!(
            run.drain(),
            [Prime, Unprime, Start { latched: true }, Prime]
        );

        run.gesture.session_ended();
        run.hold_elapses();
        assert_eq!(run.drain(), [Start { latched: false }]);
    }

    #[test]
    fn losing_the_backend_unprimes_or_cancels() {
        let mut run = Run::new();
        run.down(0);
        let step = run.gesture.lost();
        run.take(step);
        run.down(1_000);
        run.hold_elapses();
        let step = run.gesture.lost();
        run.take(step);
        assert_eq!(
            run.drain(),
            [Prime, Unprime, Prime, Start { latched: false }, Cancel]
        );
    }

    #[test]
    fn a_click_unprimes_only_before_the_hold_starts() {
        let mut run = Run::new();
        run.down(0);
        let step = run.gesture.click();
        run.take(step);
        run.down(500);
        run.hold_elapses();
        assert!(run.gesture.click().events.is_empty());
        run.up(1_200);
        assert_eq!(
            run.drain(),
            [Prime, Unprime, Prime, Start { latched: false }, Stop]
        );
    }
}
