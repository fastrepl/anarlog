use crate::{error::Error, events::Phase};

#[cfg(target_os = "macos")]
pub use self::macos::Handler;

#[cfg(not(target_os = "macos"))]
pub use self::stub::Handler;

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::Mutex;

    use hypr_dictation_ui_macos as ui;

    use super::{Error, Phase};

    pub struct Handler {
        state: Mutex<State>,
    }

    struct State {
        phase: Phase,
        amplitude: f32,
        visible: bool,
    }

    impl Handler {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(State {
                    phase: Phase::Recording,
                    amplitude: 0.0,
                    visible: false,
                }),
            }
        }

        pub fn show(&self) -> Result<(), Error> {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            ui::show();
            ui::update_state(&ui::DictationState {
                phase: to_ui_phase(s.phase),
                amplitude: s.amplitude,
            });
            s.visible = true;
            Ok(())
        }

        pub fn hide(&self) -> Result<(), Error> {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            ui::hide();
            s.visible = false;
            Ok(())
        }

        pub fn set_phase(&self, phase: Phase) -> Result<(), Error> {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            s.phase = phase;
            if s.visible {
                ui::update_state(&ui::DictationState {
                    phase: to_ui_phase(s.phase),
                    amplitude: s.amplitude,
                });
            }
            Ok(())
        }

        pub fn update_amplitude(&self, amplitude: f32) -> Result<(), Error> {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            s.amplitude = amplitude;
            if s.visible {
                ui::update_state(&ui::DictationState {
                    phase: to_ui_phase(s.phase),
                    amplitude: s.amplitude,
                });
            }
            Ok(())
        }
    }

    fn to_ui_phase(phase: Phase) -> ui::Phase {
        match phase {
            Phase::Recording => ui::Phase::Recording,
            Phase::Processing => ui::Phase::Processing,
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod stub {
    use super::{Error, Phase};

    pub struct Handler;

    impl Handler {
        pub fn new() -> Self {
            Self
        }

        pub fn show(&self) -> Result<(), Error> {
            Ok(())
        }

        pub fn hide(&self) -> Result<(), Error> {
            Ok(())
        }

        pub fn set_phase(&self, _phase: Phase) -> Result<(), Error> {
            Ok(())
        }

        pub fn update_amplitude(&self, _amplitude: f32) -> Result<(), Error> {
            Ok(())
        }
    }
}
