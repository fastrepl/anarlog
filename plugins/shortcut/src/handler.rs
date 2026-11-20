use crate::{
    error::Error,
    events::{HotKey, Options},
};

#[cfg(target_os = "macos")]
pub use self::macos::Handler;

#[cfg(not(target_os = "macos"))]
pub use self::stub::Handler;

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use anlg_shortcut_macos as sm;
    use tauri::{AppHandle, Runtime};
    use tauri_specta::Event;

    use super::{Error, HotKey, Options};
    use crate::events::Modifier;
    use crate::events::ShortcutEvent;
    use crate::gesture::{Driver, Input};

    pub struct Handler {
        listener: Mutex<Option<sm::Listener>>,
        gestures: Mutex<Option<sm::EventTap>>,
    }

    impl Handler {
        pub fn new() -> Self {
            Self {
                listener: Mutex::new(None),
                gestures: Mutex::new(None),
            }
        }

        /// Reports the modifier-only shortcut's raw presses to the gesture driver.
        pub fn register_gestures(&self, hotkey: HotKey, driver: Arc<Driver>) -> Result<(), Error> {
            let tracker = Mutex::new(sm::ChordTracker::new(convert_hotkey(&hotkey)));
            let tap = sm::EventTap::start_filtered(move |event| {
                let mut tracker = tracker.lock().unwrap_or_else(|e| e.into_inner());
                let tracked = tracker.track(event);
                let Some(input) = tracked.input else {
                    return tracked.swallow;
                };
                let acted = driver.input(match input {
                    sm::ChordInput::Down => Input::Down,
                    sm::ChordInput::Up => Input::Up,
                    sm::ChordInput::LatchKey => Input::LatchKey,
                    sm::ChordInput::Chord => Input::Chord,
                    sm::ChordInput::Click => Input::Click,
                    sm::ChordInput::Escape => Input::Escape,
                });
                if input == sm::ChordInput::LatchKey && acted {
                    tracker.swallow_latch_key();
                }
                // The press is swallowed as the legacy listener does; Space and Escape only
                // when they latched or cancelled something.
                tracked.swallow
                    || match input {
                        sm::ChordInput::Down => true,
                        sm::ChordInput::LatchKey | sm::ChordInput::Escape => acted,
                        _ => false,
                    }
            })
            .map_err(|e| Error::TapStart(e.to_string()))?;
            *self.gestures.lock().unwrap_or_else(|e| e.into_inner()) = Some(tap);
            Ok(())
        }

        pub fn register<R: Runtime>(
            &self,
            app: AppHandle<R>,
            hotkey: HotKey,
            options: Options,
        ) -> Result<(), Error> {
            let listener = sm::Listener::start(
                convert_hotkey(&hotkey),
                convert_options(options),
                move |out| {
                    let evt = match out {
                        sm::Output::StartRecording => ShortcutEvent::Pressed,
                        sm::Output::StopRecording => ShortcutEvent::Released,
                        sm::Output::Cancel => ShortcutEvent::Cancelled,
                        sm::Output::Discard => ShortcutEvent::Discarded,
                    };
                    let _ = evt.emit(&app);
                },
            )
            .map_err(|e| Error::TapStart(e.to_string()))?;

            *self.listener.lock().unwrap_or_else(|e| e.into_inner()) = Some(listener);
            Ok(())
        }

        pub fn unregister(&self) -> Result<(), Error> {
            self.listener
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            self.gestures
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            Ok(())
        }
    }

    fn convert_hotkey(hotkey: &HotKey) -> sm::HotKey {
        let mut modifiers = sm::Modifiers::empty();
        for m in &hotkey.modifiers {
            modifiers.insert(match m {
                Modifier::Command => sm::Modifier::Command,
                Modifier::RightCommand => sm::Modifier::RightCommand,
                Modifier::Option => sm::Modifier::Option,
                Modifier::Shift => sm::Modifier::Shift,
                Modifier::Control => sm::Modifier::Control,
                Modifier::Fn => sm::Modifier::Fn,
            });
        }
        sm::HotKey::new(hotkey.key, modifiers)
    }

    fn convert_options(options: Options) -> sm::Options {
        sm::Options {
            use_double_tap_only: options.use_double_tap_only,
            double_tap_lock_enabled: options.double_tap_lock_enabled,
            minimum_key_time: Duration::from_millis(options.minimum_key_time_ms),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod stub {
    use tauri::{AppHandle, Runtime};

    use super::{Error, HotKey, Options};

    pub struct Handler;

    impl Handler {
        pub fn new() -> Self {
            Self
        }

        pub fn register<R: Runtime>(
            &self,
            _app: AppHandle<R>,
            _hotkey: HotKey,
            _options: Options,
        ) -> Result<(), Error> {
            Err(Error::Unsupported)
        }

        pub fn unregister(&self) -> Result<(), Error> {
            Ok(())
        }
    }
}
