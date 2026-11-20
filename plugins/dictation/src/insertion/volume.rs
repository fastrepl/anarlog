//! Lowers other audio while dictation listens and brings it back after.
//!
//! Follows Char's `desktop-dictation/src/main/duck-audio.ts` (reference only): a volume the
//! person changed meanwhile is theirs, and so is a switch to another output, so restoring leaves
//! either alone. Headsets such as AirPods keep one volume for playback and another while their
//! microphone is open; both are lowered and both come back up. Only output volume is touched,
//! never an input device or route.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use crate::ducking::OutputDucker;

/// While someone dictates, other audio plays at this share of its volume: faint, not paused.
const DUCKED_SHARE: f32 = 0.1;
/// How long after ducking or restoring the output is watched for a headset switching profiles.
const SETTLE: Duration = Duration::from_secs(4);
const POLL: Duration = Duration::from_millis(250);
/// How long a lowered output gets to report its new volume; Bluetooth applies it a moment later.
const APPLY_WAIT: Duration = Duration::from_millis(200);
const APPLY_POLL: Duration = Duration::from_millis(10);
const SAME_LEVEL: f32 = 0.005;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Identifies the default output device.
    pub device: String,
    /// Tells apart the volumes one device keeps, such as a headset's microphone profile.
    pub profile: Option<String>,
}

/// The default output device's volume. Implementations write only to the device they name.
pub trait SystemVolume: Send + Sync + 'static {
    fn output(&self) -> Option<Output>;
    /// 0–1, or `None` while muted or without a volume control.
    fn get(&self) -> Option<f32>;
    /// Sets the volume only while `device` is still the default output.
    fn set(&self, device: &str, volume: f32) -> bool;
}

pub fn output_ducker() -> Arc<dyn OutputDucker> {
    match super::platform::system_volume() {
        Some(volume) => Arc::new(Ducker::new(volume, SETTLE, POLL)),
        None => Arc::new(crate::ducking::NoDucker),
    }
}

#[derive(Debug, Clone, Copy)]
struct Level {
    from: f32,
    to: f32,
}

struct Lowered {
    device: String,
    levels: HashMap<Option<String>, Level>,
    listening: bool,
    until: Instant,
}

struct State {
    holders: usize,
    lowered: Option<Lowered>,
    watch: u64,
}

pub struct Ducker {
    inner: Arc<Inner>,
}

struct Inner {
    volume: Box<dyn SystemVolume>,
    state: Mutex<State>,
    settle: Duration,
    poll: Duration,
}

impl Ducker {
    pub fn new(volume: Box<dyn SystemVolume>, settle: Duration, poll: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                volume,
                state: Mutex::new(State {
                    holders: 0,
                    lowered: None,
                    watch: 0,
                }),
                settle,
                poll,
            }),
        }
    }
}

struct Guard(Weak<Inner>);

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(inner) = self.0.upgrade() {
            inner.release();
        }
    }
}

impl OutputDucker for Ducker {
    fn duck(&self) -> Option<Box<dyn Send>> {
        self.inner.hold();
        Some(Box::new(Guard(Arc::downgrade(&self.inner))))
    }
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn hold(self: &Arc<Self>) {
        let mut state = self.lock();
        state.holders += 1;
        if state.holders > 1 || state.lowered.as_ref().is_some_and(|l| l.listening) {
            return;
        }
        let Some(output) = self.volume.output() else {
            return;
        };
        let from = self.volume.get();
        // Once the last watch ran out, what it lowered is no longer this ducker's to restore.
        let fresh = state
            .lowered
            .as_ref()
            .is_none_or(|l| l.device != output.device || Instant::now() > l.until);
        if fresh {
            state.lowered = Some(Lowered {
                device: output.device.clone(),
                levels: HashMap::new(),
                listening: true,
                until: Instant::now(),
            });
        }
        let lowered = state.lowered.as_mut().expect("set above");
        lowered.listening = true;
        lowered.until = Instant::now() + self.settle;
        // Still lowered from a dictation that just ended counts as lowered.
        if let Some(from) = from
            && !lowered
                .levels
                .get(&output.profile)
                .is_some_and(|level| same(level.to, from))
        {
            lower(&*self.volume, lowered, &output, from);
        }
        drop(state);
        self.watch();
    }

    fn release(self: &Arc<Self>) {
        let mut state = self.lock();
        state.holders = state.holders.saturating_sub(1);
        if state.holders > 0 {
            return;
        }
        if let Some(lowered) = state.lowered.as_mut() {
            lowered.listening = false;
            lowered.until = Instant::now() + self.settle;
        }
        self.check(&mut state);
        drop(state);
        self.watch();
    }

    /// While listening, lowers a profile the headset switched to; afterwards, brings back up
    /// the profile now playing if this lowered it and nobody changed it since.
    fn check(&self, state: &mut State) {
        let Some(lowered) = state.lowered.as_mut() else {
            return;
        };
        let output = self.volume.output();
        if output.as_ref().map(|output| &output.device) != Some(&lowered.device) {
            state.lowered = None;
            return;
        }
        let output = output.expect("checked above");
        let level = self.volume.get();
        if lowered.listening {
            if let Some(level) = level
                && !lowered.levels.contains_key(&output.profile)
            {
                lower(&*self.volume, lowered, &output, level);
            }
            return;
        }
        if let (Some(mine), Some(level)) = (lowered.levels.get(&output.profile).copied(), level)
            && same(mine.to, level)
        {
            self.volume.set(&output.device, mine.from);
            lowered.levels.remove(&output.profile);
        } else if level.is_some() {
            // Changed by the person: theirs now.
            lowered.levels.remove(&output.profile);
        }
        if lowered.levels.is_empty() || Instant::now() > lowered.until {
            state.lowered = None;
        }
    }

    fn watch(self: &Arc<Self>) {
        let generation = {
            let mut state = self.lock();
            if state.lowered.is_none() || self.settle.is_zero() {
                return;
            }
            state.watch += 1;
            state.watch
        };
        let inner = Arc::downgrade(self);
        let poll = self.poll;
        std::thread::Builder::new()
            .name("dictation-output-ducking".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(poll);
                    let Some(inner) = inner.upgrade() else { return };
                    let mut state = inner.lock();
                    if state.watch != generation {
                        return;
                    }
                    if state
                        .lowered
                        .as_ref()
                        .is_none_or(|lowered| Instant::now() > lowered.until)
                    {
                        if state.lowered.as_ref().is_some_and(|l| !l.listening) {
                            state.lowered = None;
                        }
                        return;
                    }
                    inner.check(&mut state);
                }
            })
            .ok();
    }
}

fn lower(volume: &dyn SystemVolume, lowered: &mut Lowered, output: &Output, from: f32) {
    let to = (from * DUCKED_SHARE).max(0.01);
    if to >= from || !volume.set(&output.device, to) {
        return;
    }
    // The output rounds what it was set to, so remember what it actually reports.
    let deadline = Instant::now() + APPLY_WAIT;
    let mut reported = volume.get();
    while reported.is_some_and(|level| same(level, from)) && Instant::now() < deadline {
        std::thread::sleep(APPLY_POLL);
        reported = volume.get();
    }
    let to = match reported {
        Some(level) if !same(level, from) => level,
        _ => to,
    };
    lowered
        .levels
        .insert(output.profile.clone(), Level { from, to });
}

fn same(a: f32, b: f32) -> bool {
    (a - b).abs() < SAME_LEVEL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        state: Mutex<FakeState>,
    }

    #[derive(Default)]
    struct FakeState {
        device: String,
        profile: Option<String>,
        volumes: HashMap<(String, Option<String>), f32>,
        muted: bool,
    }

    impl Fake {
        fn with(device: &str, volume: f32) -> Arc<Self> {
            let fake = Arc::new(Self::default());
            fake.switch(device, None, volume);
            fake
        }

        fn switch(&self, device: &str, profile: Option<&str>, volume: f32) {
            let mut state = self.state.lock().unwrap();
            state.device = device.into();
            state.profile = profile.map(Into::into);
            state
                .volumes
                .entry((device.into(), profile.map(Into::into)))
                .or_insert(volume);
        }

        fn person_sets(&self, volume: f32) {
            let mut state = self.state.lock().unwrap();
            let key = (state.device.clone(), state.profile.clone());
            state.volumes.insert(key, volume);
        }

        fn volume_of(&self, device: &str, profile: Option<&str>) -> f32 {
            self.state.lock().unwrap().volumes[&(device.into(), profile.map(Into::into))]
        }
    }

    impl SystemVolume for Arc<Fake> {
        fn output(&self) -> Option<Output> {
            let state = self.state.lock().unwrap();
            Some(Output {
                device: state.device.clone(),
                profile: state.profile.clone(),
            })
        }

        fn get(&self) -> Option<f32> {
            let state = self.state.lock().unwrap();
            (!state.muted).then(|| state.volumes[&(state.device.clone(), state.profile.clone())])
        }

        fn set(&self, device: &str, volume: f32) -> bool {
            let mut state = self.state.lock().unwrap();
            if state.device != device {
                return false;
            }
            let key = (state.device.clone(), state.profile.clone());
            state.volumes.insert(key, volume);
            true
        }
    }

    fn ducker(fake: &Arc<Fake>) -> Ducker {
        Ducker::new(
            Box::new(fake.clone()),
            Duration::ZERO,
            Duration::from_millis(1),
        )
    }

    #[test]
    fn lowers_other_audio_while_held_and_restores_it_after() {
        let fake = Fake::with("speakers", 0.8);
        let ducker = ducker(&fake);
        let guard = ducker.duck();
        assert!((fake.volume_of("speakers", None) - 0.08).abs() < 1e-6);
        drop(guard);
        assert_eq!(fake.volume_of("speakers", None), 0.8);
    }

    #[test]
    fn a_volume_the_person_changed_meanwhile_is_kept() {
        let fake = Fake::with("speakers", 0.8);
        let ducker = ducker(&fake);
        let guard = ducker.duck();
        fake.person_sets(0.5);
        drop(guard);
        assert_eq!(fake.volume_of("speakers", None), 0.5);
    }

    #[test]
    fn switching_outputs_meanwhile_restores_nothing_on_either() {
        let fake = Fake::with("speakers", 0.8);
        let ducker = ducker(&fake);
        let guard = ducker.duck();
        fake.switch("headphones", None, 0.6);
        drop(guard);
        assert_eq!(fake.volume_of("headphones", None), 0.6);
        assert!((fake.volume_of("speakers", None) - 0.08).abs() < 1e-6);
    }

    #[test]
    fn stays_lowered_until_the_last_holder_lets_go() {
        let fake = Fake::with("speakers", 0.5);
        let ducker = ducker(&fake);
        let first = ducker.duck();
        let second = ducker.duck();
        drop(first);
        assert!((fake.volume_of("speakers", None) - 0.05).abs() < 1e-6);
        drop(second);
        assert_eq!(fake.volume_of("speakers", None), 0.5);
    }

    #[test]
    fn a_muted_output_is_left_alone() {
        let fake = Fake::with("speakers", 0.8);
        fake.state.lock().unwrap().muted = true;
        let ducker = ducker(&fake);
        drop(ducker.duck());
        fake.state.lock().unwrap().muted = false;
        assert_eq!(fake.volume_of("speakers", None), 0.8);
    }

    #[test]
    fn a_headset_profile_switch_is_lowered_and_both_profiles_come_back() {
        let fake = Fake::with("airpods", 0.7);
        let ducker = Ducker::new(
            Box::new(fake.clone()),
            Duration::from_secs(5),
            Duration::from_millis(1),
        );
        let guard = ducker.duck();
        // The microphone opening moves the headset to its own volume.
        fake.switch("airpods", Some("headset"), 0.9);
        wait_until(|| (fake.volume_of("airpods", Some("headset")) - 0.09).abs() < 1e-6);
        drop(guard);
        assert_eq!(fake.volume_of("airpods", Some("headset")), 0.9);
        fake.switch("airpods", None, 0.0);
        wait_until(|| fake.volume_of("airpods", None) == 0.7);
    }

    fn wait_until(done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !done() {
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
