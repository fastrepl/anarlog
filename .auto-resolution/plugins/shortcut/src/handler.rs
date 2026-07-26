use crate::{
    error::Error,
    events::{HotKey, Modifier, Options},
};

#[cfg(target_os = "macos")]
pub use self::macos::Handler;

#[cfg(not(target_os = "macos"))]
pub use self::stub::Handler;

#[cfg(target_os = "macos")]
mod macos {
    use std::{sync::Mutex, time::Duration};

    use hypr_shortcut_macos as sm;
    use tauri::{AppHandle, Runtime};
    use tauri_specta::Event;

    use super::{Error, HotKey, Modifier, Options};
    use crate::events::ShortcutEvent;

    pub struct Handler {
        listener: Mutex<Option<sm::Listener>>,
    }

    impl Handler {
        pub fn new() -> Self {
            Self {
                listener: Mutex::new(None),
            }
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
            Ok(())
        }
    }

    fn convert_hotkey(hotkey: &HotKey) -> sm::HotKey {
        let mut modifiers = sm::Modifiers::empty();
        for m in &hotkey.modifiers {
            modifiers.insert(match m {
                Modifier::Command => sm::Modifier::Command,
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
