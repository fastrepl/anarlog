use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    decision::{CANCEL_WINDOW, DEFAULT_MIN_KEY_TIME, DOUBLE_TAP_WINDOW, MODIFIER_ONLY_MIN},
    hotkey::HotKey,
    key_event::KeyEvent,
};

pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub use_double_tap_only: bool,
    pub double_tap_lock_enabled: bool,
    pub minimum_key_time: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            use_double_tap_only: false,
            double_tap_lock_enabled: true,
            minimum_key_time: DEFAULT_MIN_KEY_TIME,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    PressAndHold { start_time: Instant },
    DoubleTapLock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    StartRecording,
    StopRecording,
    Cancel,
    Discard,
}

pub struct HotKeyProcessor {
    hotkey: HotKey,
    options: Options,
    state: State,
    last_tap_at: Option<Instant>,
    dirty: bool,
    clock: Arc<dyn Clock>,
}

impl HotKeyProcessor {
    pub fn new(hotkey: HotKey) -> Self {
        Self::with_clock(hotkey, Arc::new(SystemClock))
    }

    pub fn with_clock(hotkey: HotKey, clock: Arc<dyn Clock>) -> Self {
        Self {
            hotkey,
            options: Options::default(),
            state: State::Idle,
            last_tap_at: None,
            dirty: false,
            clock,
        }
    }

    pub fn hotkey(&self) -> HotKey {
        self.hotkey
    }

    pub fn options(&self) -> Options {
        self.options
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn is_matched(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    pub fn set_hotkey(&mut self, hotkey: HotKey) {
        self.hotkey = hotkey;
        self.reset();
    }

    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.last_tap_at = None;
        self.dirty = false;
    }

    pub fn process_key(&mut self, event: KeyEvent) -> Option<Output> {
        if event.is_escape() && !matches!(self.state, State::Idle) {
            self.dirty = true;
            self.state = State::Idle;
            self.last_tap_at = None;
            return Some(Output::Cancel);
        }

        if self.dirty {
            if self.chord_fully_released(&event) {
                self.dirty = false;
            } else {
                return None;
            }
        }

        if self.chord_matches_hotkey(&event) {
            self.handle_matching_chord()
        } else {
            if self.chord_is_dirty(&event) {
                self.dirty = true;
            }
            self.handle_nonmatching_chord(&event)
        }
    }

    pub fn process_mouse_click(&mut self) -> Option<Output> {
        if self.hotkey.key.is_some() {
            return None;
        }
        match self.state {
            State::Idle => None,
            State::PressAndHold { start_time } => {
                let elapsed = self.clock.now().saturating_duration_since(start_time);
                let effective_minimum = self.modifier_only_threshold();
                if elapsed < effective_minimum {
                    self.dirty = true;
                    self.state = State::Idle;
                    self.last_tap_at = None;
                    Some(Output::Discard)
                } else {
                    None
                }
            }
            State::DoubleTapLock => None,
        }
    }

    fn modifier_only_threshold(&self) -> Duration {
        self.options.minimum_key_time.max(MODIFIER_ONLY_MIN)
    }

    fn is_double_tap_only_for_current_hotkey(&self) -> bool {
        self.options.use_double_tap_only
            && self.options.double_tap_lock_enabled
            && self.hotkey.key.is_some()
    }

    fn handle_matching_chord(&mut self) -> Option<Output> {
        match self.state {
            State::Idle => {
                if self.is_double_tap_only_for_current_hotkey() {
                    self.last_tap_at = Some(self.clock.now());
                    None
                } else {
                    self.state = State::PressAndHold {
                        start_time: self.clock.now(),
                    };
                    Some(Output::StartRecording)
                }
            }
            State::PressAndHold { .. } => None,
            State::DoubleTapLock => {
                self.state = State::Idle;
                self.last_tap_at = None;
                Some(Output::StopRecording)
            }
        }
    }

    fn handle_nonmatching_chord(&mut self, event: &KeyEvent) -> Option<Output> {
        match self.state {
            State::Idle => {
                if self.is_double_tap_only_for_current_hotkey()
                    && self.chord_fully_released(event)
                    && self.last_tap_at.is_some()
                {
                    if let Some(prev) = self.last_tap_at
                        && self.clock.now().saturating_duration_since(prev) < DOUBLE_TAP_WINDOW
                    {
                        self.state = State::DoubleTapLock;
                        return Some(Output::StartRecording);
                    }
                    self.last_tap_at = None;
                }
                None
            }
            State::PressAndHold { start_time } => {
                if self.is_release_for_active_hotkey(event) {
                    if self.options.double_tap_lock_enabled
                        && let Some(prev) = self.last_tap_at
                        && self.clock.now().saturating_duration_since(prev) < DOUBLE_TAP_WINDOW
                    {
                        self.state = State::DoubleTapLock;
                        return None;
                    }
                    self.state = State::Idle;
                    self.last_tap_at = if self.options.double_tap_lock_enabled {
                        Some(self.clock.now())
                    } else {
                        None
                    };
                    Some(Output::StopRecording)
                } else {
                    let elapsed = self.clock.now().saturating_duration_since(start_time);
                    if self.hotkey.key.is_none() {
                        let threshold = self.modifier_only_threshold();
                        if elapsed < threshold {
                            self.dirty = true;
                            self.state = State::Idle;
                            self.last_tap_at = None;
                            Some(Output::Discard)
                        } else {
                            None
                        }
                    } else if elapsed < CANCEL_WINDOW {
                        self.dirty = true;
                        self.state = State::Idle;
                        self.last_tap_at = None;
                        if elapsed < self.options.minimum_key_time {
                            Some(Output::Discard)
                        } else {
                            Some(Output::StopRecording)
                        }
                    } else {
                        None
                    }
                }
            }
            State::DoubleTapLock => {
                if self.is_double_tap_only_for_current_hotkey() && self.chord_fully_released(event)
                {
                    self.state = State::Idle;
                    self.last_tap_at = None;
                    return Some(Output::StopRecording);
                }
                None
            }
        }
    }

    fn chord_matches_hotkey(&self, event: &KeyEvent) -> bool {
        if self.hotkey.key.is_some() {
            event.key == self.hotkey.key && event.modifiers.matches_exactly(self.hotkey.modifiers)
        } else {
            event.key.is_none() && event.modifiers.matches_exactly(self.hotkey.modifiers)
        }
    }

    fn chord_is_dirty(&self, event: &KeyEvent) -> bool {
        if self.hotkey.key.is_none() {
            event.key.is_some() || !event.modifiers.is_subset_of(self.hotkey.modifiers)
        } else {
            let is_subset = event.modifiers.is_subset_of(self.hotkey.modifiers);
            let is_wrong_key = event.key.is_some() && event.key != self.hotkey.key;
            !is_subset || is_wrong_key
        }
    }

    fn chord_fully_released(&self, event: &KeyEvent) -> bool {
        event.key.is_none() && event.modifiers.is_empty()
    }

    fn is_release_for_active_hotkey(&self, event: &KeyEvent) -> bool {
        if self.hotkey.key.is_some() {
            let key_released = event.key.is_none();
            let mods_subset = event.modifiers.is_subset_of(self.hotkey.modifiers);
            key_released && mods_subset
        } else {
            event.key.is_none() && !self.hotkey.modifiers.is_subset_of(event.modifiers)
        }
    }
}
