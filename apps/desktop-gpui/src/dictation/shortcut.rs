//! The dictation shortcut, as shipping's `plugins/shortcut` registers it: an accelerator such
//! as `Control+Alt+Space` through the shared global-hotkey registration, or on macOS the Fn /
//! Right Command keys through the native event tap (`crates/shortcut-macos`). Escape (and the
//! shortcut's modifiers with Escape) cancel only while a dictation is active.

use std::{
    cell::RefCell,
    rc::Rc,
    str::FromStr,
    sync::atomic::{AtomicBool, Ordering},
};

use desktop_runtime::{Result, ServiceError};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use global_hotkey::hotkey::{Code, HotKey};

use crate::platform::shortcuts::{NativeShortcuts, Shortcut, ShortcutAdapter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutEvent {
    Pressed,
    Released,
    Cancelled,
}

/// The OS side of the shortcut; tests use a recording fake.
pub trait Shortcuts {
    fn configure(&mut self, shortcut: Option<&str>) -> Result<()>;
    fn set_active(&mut self, active: bool) -> Result<()>;
}

const SHORTCUT_ID: &str = "dictation";
const CANCEL_ID: &str = "dictation-cancel";
const MODIFIED_CANCEL_ID: &str = "dictation-cancel-modified";

fn failed(message: impl Into<String>) -> ServiceError {
    ServiceError::Failed(message.into().into())
}

/// Keys only the macOS event tap can observe.
fn native_only(shortcut: &str) -> bool {
    cfg!(target_os = "macos") && matches!(shortcut, "Fn" | "RightCommand")
}

fn parse(shortcut: &str) -> Result<HotKey> {
    let key = HotKey::from_str(shortcut)
        .map_err(|error| failed(format!("Invalid dictation shortcut: {error}")))?;
    if key.mods.is_empty() {
        return Err(failed(
            "Choose a shortcut with Control, Alt, Shift, or Command.",
        ));
    }
    if key.key == Code::Escape {
        return Err(failed(
            "Escape is reserved for cancelling dictation. Choose another key.",
        ));
    }
    Ok(key)
}

/// Shipping's `validate` command, without registering anything.
pub fn validate(shortcut: &str) -> Result<()> {
    if native_only(shortcut) {
        return Ok(());
    }
    parse(shortcut).map(|_| ())
}

/// The global bindings while dictation is armed; cancel keys only while it is active.
fn bindings(shortcut: Option<&str>, active: bool) -> Vec<Shortcut> {
    let binding = |id: &str, accelerator: String| Shortcut {
        id: id.into(),
        accelerator: accelerator.into(),
        active_during_capture_only: false,
    };
    let mut bindings = Vec::new();
    let registered = shortcut.filter(|shortcut| !native_only(shortcut));
    if let Some(shortcut) = registered {
        bindings.push(binding(SHORTCUT_ID, shortcut.into()));
    }
    if active {
        bindings.push(binding(CANCEL_ID, "Escape".into()));
        if let Some((modifiers, _)) = registered.and_then(|shortcut| shortcut.rsplit_once('+')) {
            bindings.push(binding(MODIFIED_CANCEL_ID, format!("{modifiers}+Escape")));
        }
    }
    bindings
}

/// Routes the shared global-hotkey handler's dictation events; `false` for other shortcuts.
/// Repeated presses and releases are dropped, like shipping's `down.swap`.
pub fn router(sender: UnboundedSender<ShortcutEvent>) -> impl Fn(&str, bool) -> bool + Send + Sync {
    let down = AtomicBool::new(false);
    move |id, pressed| {
        let event = match id {
            SHORTCUT_ID => {
                if down.swap(pressed, Ordering::SeqCst) == pressed {
                    return true;
                }
                if pressed {
                    ShortcutEvent::Pressed
                } else {
                    ShortcutEvent::Released
                }
            }
            CANCEL_ID | MODIFIED_CANCEL_ID if pressed => ShortcutEvent::Cancelled,
            CANCEL_ID | MODIFIED_CANCEL_ID => return true,
            _ => return false,
        };
        let _ = sender.unbounded_send(event);
        true
    }
}

pub fn channel() -> (
    UnboundedSender<ShortcutEvent>,
    UnboundedReceiver<ShortcutEvent>,
) {
    unbounded()
}

/// The application's native shortcut registration, handed to dictation once at startup.
pub struct NativeHotkeys {
    pub shortcuts: Rc<RefCell<NativeShortcuts>>,
    /// The application's own bindings, kept when dictation changes its keys.
    pub base: Vec<Shortcut>,
    pub sender: UnboundedSender<ShortcutEvent>,
    pub events: UnboundedReceiver<ShortcutEvent>,
}

impl gpui::Global for NativeHotkeys {}

pub struct NativePort {
    shortcuts: Rc<RefCell<NativeShortcuts>>,
    base: Vec<Shortcut>,
    sender: UnboundedSender<ShortcutEvent>,
    shortcut: Option<String>,
    active: bool,
    #[cfg(target_os = "macos")]
    listener: Option<anlg_shortcut_macos::Listener>,
}

impl NativePort {
    pub fn new(hotkeys: NativeHotkeys) -> (Self, UnboundedReceiver<ShortcutEvent>) {
        (
            Self {
                shortcuts: hotkeys.shortcuts,
                base: hotkeys.base,
                sender: hotkeys.sender,
                shortcut: None,
                active: false,
                #[cfg(target_os = "macos")]
                listener: None,
            },
            hotkeys.events,
        )
    }

    fn apply(&mut self, shortcut: Option<&str>, active: bool) -> Result<()> {
        let mut all = self.base.clone();
        all.extend(bindings(shortcut, active));
        self.shortcuts.borrow_mut().replace(&all)
    }

    #[cfg(target_os = "macos")]
    fn listen(&mut self, shortcut: &str) -> Result<()> {
        use anlg_shortcut_macos::{HotKey, Modifier, Modifiers, Options, Output};
        let modifier = if shortcut == "Fn" {
            Modifier::Fn
        } else {
            Modifier::RightCommand
        };
        let sender = self.sender.clone();
        let listener = anlg_shortcut_macos::Listener::start(
            HotKey::new(None, Modifiers::from_slice(&[modifier])),
            Options {
                double_tap_lock_enabled: false,
                ..Options::default()
            },
            move |output| {
                let _ = sender.unbounded_send(match output {
                    Output::StartRecording => ShortcutEvent::Pressed,
                    Output::StopRecording => ShortcutEvent::Released,
                    Output::Cancel | Output::Discard => ShortcutEvent::Cancelled,
                });
            },
        )
        .map_err(|error| failed(error.to_string()))?;
        self.listener = Some(listener);
        Ok(())
    }
}

impl Shortcuts for NativePort {
    fn configure(&mut self, shortcut: Option<&str>) -> Result<()> {
        if let Some(shortcut) = shortcut {
            validate(shortcut)?;
        }
        self.active = false;
        #[cfg(target_os = "macos")]
        self.listener.take();
        self.shortcut = None;
        self.apply(None, false)?;
        let Some(shortcut) = shortcut else {
            return Ok(());
        };
        #[cfg(target_os = "macos")]
        if native_only(shortcut) {
            self.listen(shortcut)?;
            self.shortcut = Some(shortcut.into());
            return Ok(());
        }
        self.apply(Some(shortcut), false).map_err(|error| {
            failed(format!(
                "Could not register dictation shortcut. It may be used by another app: {error}"
            ))
        })?;
        self.shortcut = Some(shortcut.into());
        Ok(())
    }

    fn set_active(&mut self, active: bool) -> Result<()> {
        if self.active == active {
            return Ok(());
        }
        let shortcut = self.shortcut.clone();
        self.apply(shortcut.as_deref(), active)?;
        self.active = active;
        Ok(())
    }
}

/// Used when the platform has no global shortcut registration (for example Wayland).
pub struct UnavailableShortcuts;

impl Shortcuts for UnavailableShortcuts {
    fn configure(&mut self, shortcut: Option<&str>) -> Result<()> {
        match shortcut {
            None => Ok(()),
            Some(_) => Err(failed(
                "Global shortcuts are unavailable in this desktop session.",
            )),
        }
    }

    fn set_active(&mut self, _: bool) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_and_reserved_shortcuts() {
        for shortcut in ["", "KeyD", "not-a-shortcut", "Control+Escape"] {
            assert!(validate(shortcut).is_err(), "{shortcut}");
        }
        assert!(validate("Control+Alt+Space").is_ok());
        assert!(validate("Super+Shift+KeyD").is_ok());
        assert_eq!(validate("Fn").is_ok(), cfg!(target_os = "macos"));
    }

    #[test]
    fn cancel_keys_are_registered_only_while_dictating() {
        let ids = |bindings: Vec<Shortcut>| {
            bindings
                .into_iter()
                .map(|binding| format!("{}={}", binding.id, binding.accelerator))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(bindings(Some("Control+Alt+Space"), false)),
            ["dictation=Control+Alt+Space"]
        );
        assert_eq!(
            ids(bindings(Some("Control+Alt+Space"), true)),
            [
                "dictation=Control+Alt+Space",
                "dictation-cancel=Escape",
                "dictation-cancel-modified=Control+Alt+Escape"
            ]
        );
        assert!(bindings(None, false).is_empty());
        for binding in bindings(Some("Control+Alt+Space"), true) {
            assert!(HotKey::from_str(&binding.accelerator).is_ok());
        }
        if cfg!(target_os = "macos") {
            assert_eq!(ids(bindings(Some("Fn"), true)), ["dictation-cancel=Escape"]);
        }
    }

    #[test]
    fn router_forwards_dictation_keys_once_and_ignores_other_shortcuts() {
        let (sender, mut events) = channel();
        let route = router(sender);
        assert!(!route("show", true));
        assert!(route(SHORTCUT_ID, true));
        assert!(route(SHORTCUT_ID, true));
        assert!(route(SHORTCUT_ID, false));
        assert!(route(CANCEL_ID, false));
        assert!(route(MODIFIED_CANCEL_ID, true));
        let mut received = Vec::new();
        while let Ok(event) = events.try_recv() {
            received.push(event);
        }
        assert_eq!(
            received,
            [
                ShortcutEvent::Pressed,
                ShortcutEvent::Released,
                ShortcutEvent::Cancelled
            ]
        );
    }
}
