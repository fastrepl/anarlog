use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use shortcut_macos::{
    Clock, HotKey, HotKeyProcessor, KeyEvent, Modifier, Modifiers, Options, Output, State,
};

// ---------- Carbon kVK_* keycodes used in tests ----------
const K_A: u16 = 0x00;
const K_B: u16 = 0x0B;
const K_C: u16 = 0x08;
const K_T: u16 = 0x11;
const K_U: u16 = 0x20;
const K_ESC: u16 = 0x35;

struct MockClock {
    start: Instant,
    offset_ms: Mutex<u64>,
}

impl MockClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            start: Instant::now(),
            offset_ms: Mutex::new(0),
        })
    }

    fn set(&self, secs: f64) {
        *self.offset_ms.lock().unwrap() = (secs * 1000.0) as u64;
    }
}

impl Clock for MockClock {
    fn now(&self) -> Instant {
        let ms = *self.offset_ms.lock().unwrap();
        self.start + Duration::from_millis(ms)
    }
}

#[derive(Debug, Clone, Copy)]
struct Step {
    time: f64,
    key: Option<u16>,
    modifiers: &'static [Modifier],
    expected_output: Option<Output>,
    expected_matched: Option<bool>,
    expected_state: Option<StateKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum StateKind {
    Idle,
    PressAndHold,
    DoubleTapLock,
}

impl StateKind {
    fn matches(self, state: State) -> bool {
        match (self, state) {
            (Self::Idle, State::Idle) => true,
            (Self::PressAndHold, State::PressAndHold { .. }) => true,
            (Self::DoubleTapLock, State::DoubleTapLock) => true,
            _ => false,
        }
    }
}

fn step(time: f64, key: Option<u16>, modifiers: &'static [Modifier]) -> Step {
    Step {
        time,
        key,
        modifiers,
        expected_output: None,
        expected_matched: None,
        expected_state: None,
    }
}

impl Step {
    fn out(mut self, o: Output) -> Self {
        self.expected_output = Some(o);
        self
    }
    fn matched(mut self, m: bool) -> Self {
        self.expected_matched = Some(m);
        self
    }
    fn state(mut self, s: StateKind) -> Self {
        self.expected_state = Some(s);
        self
    }
}

struct Scenario {
    hotkey: HotKey,
    options: Options,
    steps: Vec<Step>,
}

impl Scenario {
    fn new(hotkey: HotKey) -> Self {
        Self {
            hotkey,
            options: Options::default(),
            steps: Vec::new(),
        }
    }

    fn options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    fn add(mut self, s: Step) -> Self {
        self.steps.push(s);
        self
    }

    fn run(self) {
        let clock = MockClock::new();
        let mut p = HotKeyProcessor::with_clock(self.hotkey, clock.clone());
        p.set_options(self.options);

        for (idx, step) in self.steps.iter().enumerate() {
            clock.set(step.time);
            let event = KeyEvent::new(step.key, Modifiers::from(step.modifiers));
            let actual = p.process_key(event);
            assert_eq!(
                actual, step.expected_output,
                "step #{} @ {}s: output mismatch",
                idx, step.time
            );
            if let Some(exp) = step.expected_matched {
                assert_eq!(
                    p.is_matched(),
                    exp,
                    "step #{} @ {}s: is_matched mismatch",
                    idx,
                    step.time
                );
            }
            if let Some(exp) = step.expected_state {
                assert!(
                    exp.matches(p.state()),
                    "step #{} @ {}s: state mismatch; got {:?}",
                    idx,
                    step.time,
                    p.state()
                );
            }
        }
    }
}

fn cmd_a() -> HotKey {
    HotKey::new(Some(K_A), Modifiers::from([Modifier::Command]))
}

fn opt() -> HotKey {
    HotKey::modifier_only(Modifiers::from([Modifier::Option]))
}

fn opt_cmd() -> HotKey {
    HotKey::modifier_only(Modifiers::from([Modifier::Option, Modifier::Command]))
}

fn fn_only() -> HotKey {
    HotKey::modifier_only(Modifiers::from([Modifier::Fn]))
}

// ---------- press-and-hold: starts ----------

#[test]
fn press_and_hold_starts_standard() {
    Scenario::new(cmd_a())
        .add(
            step(0.0, Some(K_A), &[Modifier::Command])
                .out(Output::StartRecording)
                .matched(true),
        )
        .run();
}

#[test]
fn press_and_hold_starts_modifier_only() {
    Scenario::new(opt())
        .add(
            step(0.0, None, &[Modifier::Option])
                .out(Output::StartRecording)
                .matched(true),
        )
        .run();
}

// ---------- press-and-hold: stops on release ----------

#[test]
fn press_and_hold_stops_on_release_standard() {
    Scenario::new(cmd_a())
        .add(step(0.0, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(
            step(0.2, None, &[Modifier::Command])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

#[test]
fn press_and_hold_stops_on_release_modifier_only() {
    Scenario::new(opt())
        .add(step(0.0, None, &[Modifier::Option]).out(Output::StartRecording))
        .add(
            step(0.2, None, &[])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

#[test]
fn press_and_hold_stops_on_release_multiple_modifiers() {
    Scenario::new(opt_cmd())
        .add(step(0.0, None, &[Modifier::Option]).matched(false))
        .add(
            step(0.1, None, &[Modifier::Option, Modifier::Command])
                .out(Output::StartRecording)
                .matched(true),
        )
        .add(
            step(0.2, None, &[])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

#[test]
fn press_and_hold_releasing_modifier_before_key_still_stops() {
    Scenario::new(HotKey::new(Some(K_U), Modifiers::from([Modifier::Option])))
        .add(step(0.0, None, &[Modifier::Option]).matched(false))
        .add(
            step(0.05, Some(K_U), &[Modifier::Option])
                .out(Output::StartRecording)
                .matched(true),
        )
        .add(step(1.5, Some(K_U), &[]).matched(true))
        .add(
            step(1.55, None, &[])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

// ---------- cancel / ignore on other keys ----------

#[test]
fn press_and_hold_cancels_on_other_key_standard() {
    Scenario::new(cmd_a())
        .add(step(0.0, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(
            step(0.5, Some(K_B), &[Modifier::Command])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

#[test]
fn press_and_hold_does_not_cancel_after_threshold_standard() {
    Scenario::new(cmd_a())
        .add(step(0.0, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(step(1.5, Some(K_B), &[Modifier::Command]).matched(true))
        .run();
}

#[test]
fn press_and_hold_ignores_extra_modifier_after_threshold_modifier_only() {
    Scenario::new(opt())
        .add(step(0.0, None, &[Modifier::Option]).out(Output::StartRecording))
        .add(step(0.5, None, &[Modifier::Option, Modifier::Command]).matched(true))
        .run();
}

#[test]
fn press_and_hold_does_not_cancel_after_threshold_modifier_only() {
    Scenario::new(opt())
        .add(step(0.0, None, &[Modifier::Option]).out(Output::StartRecording))
        .add(step(1.5, None, &[Modifier::Option, Modifier::Command]).matched(true))
        .run();
}

// ---------- dirty / backslide ----------

#[test]
fn press_and_hold_does_not_trigger_on_backslide_standard() {
    Scenario::new(cmd_a())
        .add(step(0.0, Some(K_A), &[Modifier::Command, Modifier::Shift]).matched(false))
        .add(step(0.1, Some(K_A), &[Modifier::Command]).matched(false))
        .add(step(0.2, None, &[]).matched(false))
        .add(
            step(0.3, Some(K_A), &[Modifier::Command])
                .out(Output::StartRecording)
                .matched(true),
        )
        .run();
}

#[test]
fn dirty_state_blocks_input_until_full_release() {
    Scenario::new(opt())
        .add(step(0.0, None, &[Modifier::Option]).out(Output::StartRecording))
        .add(
            step(0.1, None, &[Modifier::Option, Modifier::Command])
                .out(Output::Discard)
                .matched(false),
        )
        .add(step(0.2, None, &[Modifier::Option]).matched(false))
        .add(step(0.3, Some(K_C), &[Modifier::Option]).matched(false))
        .add(step(0.4, None, &[]).matched(false))
        .add(
            step(0.5, None, &[Modifier::Option])
                .out(Output::StartRecording)
                .matched(true),
        )
        .run();
}

#[test]
fn multiple_modifiers_no_backslide_activation() {
    Scenario::new(opt_cmd())
        .add(step(
            0.0,
            None,
            &[Modifier::Option, Modifier::Command, Modifier::Shift],
        ))
        .add(step(0.1, None, &[Modifier::Option, Modifier::Command]))
        .add(step(0.2, None, &[]))
        .add(
            step(0.3, None, &[Modifier::Option, Modifier::Command])
                .out(Output::StartRecording)
                .matched(true),
        )
        .run();
}

// ---------- double-tap lock ----------

#[test]
fn double_tap_lock_standard() {
    Scenario::new(cmd_a())
        .add(step(0.0, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(step(0.1, None, &[Modifier::Command]).out(Output::StopRecording))
        .add(step(0.1, None, &[]))
        .add(step(0.15, None, &[Modifier::Command]))
        .add(step(0.2, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(
            step(0.3, None, &[Modifier::Command])
                .matched(true)
                .state(StateKind::DoubleTapLock),
        )
        .run();
}

#[test]
fn double_tap_lock_modifier_only() {
    Scenario::new(opt())
        .add(step(0.0, None, &[Modifier::Option]).out(Output::StartRecording))
        .add(step(0.1, None, &[]).out(Output::StopRecording))
        .add(step(0.2, None, &[Modifier::Option]).out(Output::StartRecording))
        .add(
            step(0.3, None, &[])
                .matched(true)
                .state(StateKind::DoubleTapLock),
        )
        .run();
}

#[test]
fn double_tap_ignored_when_slow_standard() {
    Scenario::new(cmd_a())
        .add(step(0.0, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(step(0.1, None, &[Modifier::Command]).out(Output::StopRecording))
        .add(step(0.4, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .run();
}

#[test]
fn double_tap_lock_stops_on_next_tap_modifier_only() {
    Scenario::new(opt())
        .add(step(0.0, None, &[Modifier::Option]).out(Output::StartRecording))
        .add(step(0.1, None, &[]).out(Output::StopRecording))
        .add(step(0.2, None, &[Modifier::Option]).out(Output::StartRecording))
        .add(step(0.3, None, &[]).state(StateKind::DoubleTapLock))
        .add(
            step(1.0, None, &[Modifier::Option])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

#[test]
fn double_tap_lock_disabled_stays_press_and_hold() {
    let opts = Options {
        double_tap_lock_enabled: false,
        ..Default::default()
    };
    Scenario::new(cmd_a())
        .options(opts)
        .add(step(0.0, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(step(0.1, None, &[Modifier::Command]).out(Output::StopRecording))
        .add(step(0.1, None, &[]))
        .add(step(0.15, None, &[Modifier::Command]))
        .add(step(0.2, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(
            step(0.3, None, &[Modifier::Command])
                .out(Output::StopRecording)
                .state(StateKind::Idle),
        )
        .run();
}

// ---------- ESC ----------

#[test]
fn escape_cancels_from_hold() {
    Scenario::new(cmd_a())
        .add(step(0.0, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(
            step(0.5, Some(K_ESC), &[])
                .out(Output::Cancel)
                .matched(false),
        )
        .run();
}

#[test]
fn escape_while_holding_hotkey_does_not_restart() {
    Scenario::new(cmd_a())
        .add(step(0.0, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(
            step(0.5, Some(K_ESC), &[Modifier::Command])
                .out(Output::Cancel)
                .matched(false),
        )
        .add(step(0.6, Some(K_A), &[Modifier::Command]).matched(false))
        .add(step(0.7, None, &[]).matched(false))
        .add(
            step(0.8, Some(K_A), &[Modifier::Command])
                .out(Output::StartRecording)
                .matched(true),
        )
        .run();
}

// ---------- multiple modifiers edge cases ----------

#[test]
fn multiple_modifiers_partial_release_stops() {
    Scenario::new(opt_cmd())
        .add(step(0.0, None, &[Modifier::Option, Modifier::Command]).out(Output::StartRecording))
        .add(
            step(0.5, None, &[Modifier::Option])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

#[test]
fn multiple_modifiers_adding_extra_ignored_after_threshold() {
    Scenario::new(opt_cmd())
        .add(step(0.0, None, &[Modifier::Option, Modifier::Command]).out(Output::StartRecording))
        .add(
            step(
                0.5,
                None,
                &[Modifier::Option, Modifier::Command, Modifier::Shift],
            )
            .matched(true),
        )
        .run();
}

#[test]
fn modifier_only_does_not_trigger_with_other_keys() {
    Scenario::new(opt_cmd())
        .add(step(0.0, Some(K_T), &[Modifier::Command, Modifier::Option]))
        .add(step(0.1, None, &[Modifier::Command, Modifier::Option]))
        .add(step(0.2, None, &[]))
        .add(
            step(0.3, None, &[Modifier::Command, Modifier::Option])
                .out(Output::StartRecording)
                .matched(true),
        )
        .add(
            step(0.4, None, &[])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

#[test]
fn key_modifier_changing_modifiers_cancels_within_1s() {
    Scenario::new(cmd_a())
        .add(step(0.0, Some(K_A), &[Modifier::Command]).out(Output::StartRecording))
        .add(
            step(0.5, Some(K_A), &[Modifier::Command, Modifier::Shift])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

// ---------- fn-specific regression ----------

#[test]
fn modifier_only_fn_triggers_after_fn_plus_key_then_full_release() {
    Scenario::new(fn_only())
        .add(step(0.00, Some(K_C), &[Modifier::Fn]))
        .add(step(0.05, None, &[]))
        .add(
            step(0.20, None, &[Modifier::Fn])
                .out(Output::StartRecording)
                .matched(true),
        )
        .add(
            step(0.60, None, &[])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

#[test]
fn modifier_only_fn_does_not_trigger_when_fn_held_after_key_release() {
    Scenario::new(fn_only())
        .add(step(0.00, Some(K_C), &[Modifier::Fn]))
        .add(step(0.05, None, &[Modifier::Fn]))
        .add(step(0.10, None, &[]))
        .add(
            step(0.25, None, &[Modifier::Fn])
                .out(Output::StartRecording)
                .matched(true),
        )
        .add(
            step(0.70, None, &[])
                .out(Output::StopRecording)
                .matched(false),
        )
        .run();
}

// ---------- mouse click (separate API) ----------

#[test]
fn mouse_click_discards_modifier_only_within_threshold() {
    let clock = MockClock::new();
    let mut p = HotKeyProcessor::with_clock(opt(), clock.clone());

    clock.set(0.0);
    assert_eq!(
        p.process_key(KeyEvent::new(None, Modifiers::from([Modifier::Option]))),
        Some(Output::StartRecording)
    );

    clock.set(0.1);
    assert_eq!(p.process_mouse_click(), Some(Output::Discard));
    assert!(!p.is_matched());
}

#[test]
fn mouse_click_ignored_modifier_only_after_threshold() {
    let clock = MockClock::new();
    let mut p = HotKeyProcessor::with_clock(opt(), clock.clone());

    clock.set(0.0);
    p.process_key(KeyEvent::new(None, Modifiers::from([Modifier::Option])));

    clock.set(0.5);
    assert_eq!(p.process_mouse_click(), None);
    assert!(p.is_matched());
}

#[test]
fn mouse_click_ignored_for_standard_hotkey() {
    let clock = MockClock::new();
    let mut p = HotKeyProcessor::with_clock(cmd_a(), clock.clone());

    clock.set(0.0);
    p.process_key(KeyEvent::new(
        Some(K_A),
        Modifiers::from([Modifier::Command]),
    ));

    clock.set(0.1);
    assert_eq!(p.process_mouse_click(), None);
    assert!(p.is_matched());
}
