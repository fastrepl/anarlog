use crate::{error::Error, events::Phase};

#[cfg(target_os = "macos")]
pub use self::macos::Handler;

#[cfg(not(target_os = "macos"))]
pub use self::desktop::Handler;

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::Mutex;

    use anlg_dictation_ui_macos as ui;

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
        pub fn new(_app: tauri::AppHandle) -> Self {
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
mod desktop {
    use super::{Error, Phase};
    use tauri::Manager;

    pub struct Handler {
        app: tauri::AppHandle,
    }

    impl Handler {
        pub fn new(app: tauri::AppHandle) -> Self {
            Self { app }
        }

        pub fn show(&self) -> Result<(), Error> {
            let window = if let Some(window) = self.app.get_webview_window("dictation-overlay") {
                window
            } else {
                let window = tauri::WebviewWindowBuilder::new(
                    &self.app,
                    "dictation-overlay",
                    tauri::WebviewUrl::App("dictation.html".into()),
                )
                .title("Anarlog Dictation")
                .inner_size(240.0, 52.0)
                .decorations(false)
                .focused(false)
                .focusable(false)
                .visible(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .build()
                .map_err(|e| Error::Recording(e.to_string()))?;
                window
            };
            let monitor = self
                .app
                .cursor_position()
                .ok()
                .and_then(|cursor| {
                    self.app
                        .monitor_from_point(cursor.x, cursor.y)
                        .ok()
                        .flatten()
                })
                .or_else(|| self.app.primary_monitor().ok().flatten());
            if let Some(monitor) = monitor {
                let work = monitor.work_area();
                let size = work.size.to_logical::<f64>(monitor.scale_factor());
                let position = work.position.to_logical::<f64>(monitor.scale_factor());
                window
                    .set_position(tauri::LogicalPosition::new(
                        position.x + (size.width - 240.0) / 2.0,
                        position.y + size.height - 120.0,
                    ))
                    .map_err(|e| Error::Recording(e.to_string()))?;
            }
            window.show().map_err(|e| Error::Recording(e.to_string()))?;
            Ok(())
        }

        pub fn hide(&self) -> Result<(), Error> {
            if let Some(window) = self.app.get_webview_window("dictation-overlay") {
                window.hide().map_err(|e| Error::Recording(e.to_string()))?;
            }
            Ok(())
        }

        pub fn set_phase(&self, phase: Phase) -> Result<(), Error> {
            if let Some(window) = self.app.get_webview_window("dictation-overlay") {
                let phase = match phase {
                    Phase::Recording => "recording",
                    Phase::Processing => "processing",
                };
                window
                    .eval(&format!("document.body.dataset.phase = '{phase}'"))
                    .map_err(|e| Error::Recording(e.to_string()))?;
            }
            Ok(())
        }

        pub fn update_amplitude(&self, _amplitude: f32) -> Result<(), Error> {
            Ok(())
        }
    }
}
