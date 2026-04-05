use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    Progress,
    Traces,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputAction {
    ToggleView,
    Interrupt,
    SeekForward,
    SeekBackward,
    TogglePause,
    Up,
    Down,
    Confirm,
}

pub(crate) struct BackgroundInput {
    rx: mpsc::Receiver<InputAction>,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl BackgroundInput {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        let handle = std::thread::spawn(move || {
            while thread_running.load(Ordering::Relaxed) {
                if !event::poll(INPUT_POLL_INTERVAL).unwrap_or(false) {
                    continue;
                }

                let Ok(Event::Key(key)) = event::read() else {
                    continue;
                };

                match action_for_key(key) {
                    Some(InputAction::Interrupt) => {
                        #[cfg(unix)]
                        unsafe {
                            libc::kill(libc::getpid(), libc::SIGINT);
                        }
                        #[cfg(not(unix))]
                        std::process::exit(130);
                    }
                    Some(action) => {
                        let _ = tx.send(action);
                    }
                    None => {}
                }
            }
        });

        Self {
            rx,
            running,
            handle: Some(handle),
        }
    }

    pub fn try_recv(&self) -> Result<InputAction, mpsc::TryRecvError> {
        self.rx.try_recv()
    }

    pub fn shutdown(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn action_for_key(key: KeyEvent) -> Option<InputAction> {
    match key {
        KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(InputAction::ToggleView),
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => Some(InputAction::Interrupt),
        KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(InputAction::SeekForward),
        KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(InputAction::SeekBackward),
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(InputAction::TogglePause),
        KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            ..
        }
        | KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(InputAction::Up),
        KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            ..
        }
        | KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(InputAction::Down),
        KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            ..
        } => Some(InputAction::Confirm),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_maps_to_interrupt_action() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(action_for_key(key), Some(InputAction::Interrupt));
    }

    #[test]
    fn plain_d_maps_to_toggle_action() {
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        assert_eq!(action_for_key(key), Some(InputAction::ToggleView));
    }

    #[test]
    fn arrow_keys_map_to_seek() {
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(action_for_key(right), Some(InputAction::SeekForward));
        let left = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(action_for_key(left), Some(InputAction::SeekBackward));
    }

    #[test]
    fn space_maps_to_toggle_pause() {
        let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(action_for_key(key), Some(InputAction::TogglePause));
    }
}
