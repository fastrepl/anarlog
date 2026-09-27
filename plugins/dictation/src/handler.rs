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
    use super::{Error, Phase, phase_name};
    use std::sync::{Arc, Mutex};
    use tauri::Manager;

    pub struct Handler {
        app: tauri::AppHandle,
        phase: Arc<Mutex<Phase>>,
    }

    impl Handler {
        pub fn new(app: tauri::AppHandle) -> Self {
            Self {
                app,
                phase: Arc::new(Mutex::new(Phase::Recording)),
            }
        }

        pub fn show(&self) -> Result<(), Error> {
            // Window/monitor queries hit the windowing system; on Linux/X11 they must
            // run on the GTK main thread. run_on_main_thread runs inline when the
            // caller is already on it. The phase is read inside the closure so a
            // concurrent set_phase while queued still reaches the overlay.
            let app = self.app.clone();
            let handle = app.clone();
            let phase = self.phase.clone();
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            handle
                .run_on_main_thread(move || {
                    let phase = phase_name(*phase.lock().unwrap_or_else(|e| e.into_inner()));
                    let _ = tx.send(show_overlay(&app, phase));
                })
                .map_err(|e| Error::Recording(e.to_string()))?;
            rx.recv()
                .map_err(|_| Error::Recording("main thread reply lost".to_string()))?
        }

        pub fn hide(&self) -> Result<(), Error> {
            if let Some(window) = self.app.get_webview_window("dictation-overlay") {
                window.hide().map_err(|e| Error::Recording(e.to_string()))?;
            }
            Ok(())
        }

        pub fn set_phase(&self, phase: Phase) -> Result<(), Error> {
            *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = phase;
            if let Some(window) = self.app.get_webview_window("dictation-overlay") {
                let phase = phase_name(phase);
                window
                    .eval(&format!(
                        "if (document.body) document.body.dataset.phase = '{phase}'"
                    ))
                    .map_err(|e| Error::Recording(e.to_string()))?;
            }
            Ok(())
        }

        pub fn update_amplitude(&self, _amplitude: f32) -> Result<(), Error> {
            Ok(())
        }
    }

    fn show_overlay(app: &tauri::AppHandle, phase: &'static str) -> Result<(), Error> {
        let window = if let Some(window) = app.get_webview_window("dictation-overlay") {
            window
        } else {
            tauri::WebviewWindowBuilder::new(
                app,
                "dictation-overlay",
                tauri::WebviewUrl::App("dictation.html".into()),
            )
            .initialization_script(format!(
                "window.addEventListener('DOMContentLoaded', () => {{ document.body.dataset.phase = '{phase}'; }});",
            ))
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
            .map_err(|e| Error::Recording(e.to_string()))?
        };
        let monitor = app
            .cursor_position()
            .ok()
            .and_then(|cursor| app.monitor_from_point(cursor.x, cursor.y).ok().flatten())
            .or_else(|| app.primary_monitor().ok().flatten());
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
        window
            .set_ignore_cursor_events(true)
            .map_err(|e| Error::Recording(e.to_string()))?;
        window.show().map_err(|e| Error::Recording(e.to_string()))?;
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Recording => "recording",
        Phase::Processing => "processing",
    }
}
