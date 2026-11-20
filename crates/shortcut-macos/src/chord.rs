//! Press, release and interruption of a modifier-only shortcut (Fn, Right Command) as the event
//! tap sees them, for gesture handling above this crate. Unlike [`crate::HotKeyProcessor`] it
//! decides nothing about holds or latches; it only reports what the keys did.

use crate::{
    hotkey::{HotKey, Modifier, Modifiers},
    key_event::KeyEvent,
    tap::TapEvent,
};

pub const KVK_SPACE: u16 = 0x31;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordInput {
    Down,
    Up,
    /// Space while the shortcut is held.
    LatchKey,
    /// Another key or modifier joined the held shortcut.
    Chord,
    Click,
    Escape,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tracked {
    pub input: Option<ChordInput>,
    /// Swallow the event whatever the gesture does with it (a latched Space's repeats).
    pub swallow: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Up,
    Down,
    /// Interrupted or contaminated: nothing counts until every key is released.
    Dirty,
}

pub struct ChordTracker {
    modifiers: Modifiers,
    state: State,
    swallow_space: bool,
}

impl ChordTracker {
    /// For a modifier-only shortcut; a shortcut with a key goes through global registration.
    pub fn new(hotkey: HotKey) -> Self {
        Self {
            modifiers: hotkey.modifiers,
            state: State::Up,
            swallow_space: false,
        }
    }

    /// The Space that latched a dictation types nothing, nor do its repeats, until released.
    pub fn swallow_latch_key(&mut self) {
        self.swallow_space = true;
    }

    pub fn track(&mut self, event: TapEvent) -> Tracked {
        let key = match event {
            TapEvent::MouseClick => {
                return input((self.state == State::Down).then_some(ChordInput::Click));
            }
            TapEvent::KeyUp(event) => {
                if event.key == Some(KVK_SPACE) {
                    self.swallow_space = false;
                }
                return Tracked::default();
            }
            TapEvent::Key(event) => event,
        };
        if key.is_escape() {
            return input(Some(ChordInput::Escape));
        }
        if key.key == Some(KVK_SPACE) && self.swallow_space {
            return Tracked {
                input: None,
                swallow: true,
            };
        }
        let modifiers = self.normalize(key);
        let exact = modifiers.matches_exactly(self.modifiers);
        match self.state {
            State::Up => {
                if key.key.is_none() && exact {
                    self.state = State::Down;
                    return input(Some(ChordInput::Down));
                }
                if key.key.is_some() || !modifiers.is_subset_of(self.modifiers) {
                    self.state = State::Dirty;
                }
                Tracked::default()
            }
            State::Down => {
                if key.key == Some(KVK_SPACE) && exact {
                    return input(Some(ChordInput::LatchKey));
                }
                if key.key.is_some() {
                    self.state = State::Dirty;
                    return input(Some(ChordInput::Chord));
                }
                if !self.modifiers.is_subset_of(modifiers) {
                    self.state = if modifiers.is_empty() {
                        State::Up
                    } else {
                        State::Dirty
                    };
                    return input(Some(ChordInput::Up));
                }
                if !exact {
                    self.state = State::Dirty;
                    return input(Some(ChordInput::Chord));
                }
                Tracked::default()
            }
            State::Dirty => {
                if key.key.is_none() && modifiers.is_empty() {
                    self.state = State::Up;
                }
                Tracked::default()
            }
        }
    }

    // As HotKeyProcessor: Right Command is told apart from Command only when the shortcut uses it.
    fn normalize(&self, event: KeyEvent) -> Modifiers {
        let mut modifiers = event.modifiers;
        if self.modifiers.contains(Modifier::RightCommand) {
            if !self.modifiers.contains(Modifier::Command) {
                modifiers.remove(Modifier::Command);
            }
        } else {
            modifiers.remove(Modifier::RightCommand);
        }
        modifiers
    }
}

fn input(input: Option<ChordInput>) -> Tracked {
    Tracked {
        input,
        swallow: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const K_A: u16 = 0x00;

    fn fn_key() -> ChordTracker {
        ChordTracker::new(HotKey::modifier_only(Modifiers::from([Modifier::Fn])))
    }

    fn key(key: Option<u16>, modifiers: &[Modifier]) -> TapEvent {
        TapEvent::Key(KeyEvent::new(key, Modifiers::from_slice(modifiers)))
    }

    fn up(key: u16, modifiers: &[Modifier]) -> TapEvent {
        TapEvent::KeyUp(KeyEvent::new(Some(key), Modifiers::from_slice(modifiers)))
    }

    fn inputs(tracker: &mut ChordTracker, events: &[TapEvent]) -> Vec<ChordInput> {
        events
            .iter()
            .filter_map(|event| tracker.track(*event).input)
            .collect()
    }

    #[test]
    fn reports_press_and_release_of_the_modifier() {
        let mut tracker = fn_key();
        let got = inputs(
            &mut tracker,
            &[
                key(None, &[Modifier::Fn]),
                key(None, &[Modifier::Fn]),
                key(None, &[]),
            ],
        );
        assert_eq!(got, [ChordInput::Down, ChordInput::Up]);
    }

    #[test]
    fn another_key_or_modifier_is_a_chord_until_everything_is_released() {
        let mut tracker = fn_key();
        let got = inputs(
            &mut tracker,
            &[
                key(None, &[Modifier::Fn]),
                key(Some(K_A), &[Modifier::Fn]),
                key(None, &[]),
                key(None, &[Modifier::Fn]),
                key(None, &[Modifier::Fn, Modifier::Shift]),
                key(None, &[Modifier::Shift]),
                key(None, &[Modifier::Fn, Modifier::Shift]),
                key(None, &[]),
            ],
        );
        assert_eq!(
            got,
            [
                ChordInput::Down,
                ChordInput::Chord,
                ChordInput::Down,
                ChordInput::Chord
            ]
        );
    }

    #[test]
    fn a_modifier_already_held_never_presses_the_shortcut() {
        let mut tracker = fn_key();
        let got = inputs(
            &mut tracker,
            &[
                key(None, &[Modifier::Shift]),
                key(None, &[Modifier::Shift, Modifier::Fn]),
                key(None, &[Modifier::Fn]),
                key(None, &[]),
            ],
        );
        assert!(got.is_empty());
    }

    #[test]
    fn space_latches_and_its_repeats_are_swallowed_until_it_is_released() {
        let mut tracker = fn_key();
        assert_eq!(
            inputs(
                &mut tracker,
                &[
                    key(None, &[Modifier::Fn]),
                    key(Some(KVK_SPACE), &[Modifier::Fn])
                ]
            ),
            [ChordInput::Down, ChordInput::LatchKey]
        );
        tracker.swallow_latch_key();
        assert_eq!(tracker.track(key(None, &[])).input, Some(ChordInput::Up));
        let repeat = tracker.track(key(Some(KVK_SPACE), &[]));
        assert!(repeat.swallow && repeat.input.is_none());
        tracker.track(up(KVK_SPACE, &[]));
        assert!(!tracker.track(key(Some(KVK_SPACE), &[])).swallow);
    }

    use crate::key_event::KVK_ESCAPE;

    #[test]
    fn escape_and_clicks_are_reported() {
        let mut tracker = fn_key();
        assert_eq!(tracker.track(TapEvent::MouseClick).input, None);
        assert_eq!(
            inputs(
                &mut tracker,
                &[
                    key(Some(KVK_ESCAPE), &[]),
                    key(None, &[Modifier::Fn]),
                    TapEvent::MouseClick,
                    key(None, &[]),
                ]
            ),
            [
                ChordInput::Escape,
                ChordInput::Down,
                ChordInput::Click,
                ChordInput::Up
            ]
        );
    }

    #[test]
    fn right_command_ignores_the_shared_command_flag() {
        let mut tracker = ChordTracker::new(HotKey::modifier_only(Modifiers::from([
            Modifier::RightCommand,
        ])));
        let got = inputs(
            &mut tracker,
            &[
                key(None, &[Modifier::Command, Modifier::RightCommand]),
                key(None, &[]),
            ],
        );
        assert_eq!(got, [ChordInput::Down, ChordInput::Up]);
    }
}
