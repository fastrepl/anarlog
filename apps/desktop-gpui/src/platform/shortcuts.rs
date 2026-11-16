use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use desktop_runtime::{Result, ServiceError};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};

static NATIVE_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

pub struct NativeShortcuts {
    manager: GlobalHotKeyManager,
    active: HashMap<u32, HotKey>,
    routes: Arc<Mutex<HashMap<u32, Arc<str>>>>,
}

impl NativeShortcuts {
    /// Construct once on GPUI's platform thread, before registering any shortcuts.
    pub fn new(notify: impl Fn(Arc<str>, bool) + Send + Sync + 'static) -> Result<Self> {
        if cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return Err(ServiceError::Unsupported(
                "Wayland global shortcuts require a portal adapter".into(),
            ));
        }
        let manager = GlobalHotKeyManager::new().map_err(native_error)?;
        if NATIVE_HANDLER_INSTALLED.swap(true, Ordering::AcqRel) {
            return Err(ServiceError::Unsupported(
                "Native global shortcut event handler is already installed".into(),
            ));
        }
        let routes = Arc::new(Mutex::new(HashMap::<u32, Arc<str>>::new()));
        let events = routes.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            let id = events
                .lock()
                .ok()
                .and_then(|routes| routes.get(&event.id).cloned());
            if let Some(id) = id {
                notify(id, event.state == HotKeyState::Pressed);
            }
        }));
        Ok(Self {
            manager,
            routes,
            active: HashMap::new(),
        })
    }
}

fn native_error(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

impl ShortcutAdapter for NativeShortcuts {
    fn replace(&mut self, bindings: &[Shortcut]) -> Result<()> {
        let mut desired = HashMap::new();
        for binding in bindings {
            let hotkey = HotKey::from_str(&binding.accelerator).map_err(native_error)?;
            if desired
                .insert(hotkey.id(), (hotkey, binding.id.clone()))
                .is_some()
            {
                return Err(ServiceError::Failed(
                    "Accelerators resolve to the same native shortcut".into(),
                ));
            }
        }
        let mut routes = self.routes.lock().map_err(native_error)?;
        let mut added = Vec::new();
        for (id, (hotkey, name)) in &desired {
            if self.active.contains_key(id) {
                continue;
            }
            if let Err(error) = self.manager.register(*hotkey) {
                let mut errors = vec![error.to_string()];
                for hotkey in added {
                    match self.manager.unregister(hotkey) {
                        Ok(()) => {
                            self.active.remove(&hotkey.id());
                            routes.remove(&hotkey.id());
                        }
                        Err(error) => errors.push(format!("Shortcut rollback failed: {error}")),
                    }
                }
                return Err(native_error(errors.join("; ")));
            }
            self.active.insert(*id, *hotkey);
            routes.insert(*id, name.clone());
            added.push(*hotkey);
        }
        let removed: Vec<_> = self
            .active
            .iter()
            .filter(|(id, _)| !desired.contains_key(id))
            .map(|(_, hotkey)| *hotkey)
            .collect();
        for hotkey in removed {
            self.manager.unregister(hotkey).map_err(native_error)?;
            self.active.remove(&hotkey.id());
            routes.remove(&hotkey.id());
        }
        for (id, (_, name)) in desired {
            routes.insert(id, name);
        }
        Ok(())
    }
}

impl Drop for NativeShortcuts {
    fn drop(&mut self) {
        for hotkey in self.active.values() {
            if let Err(error) = self.manager.unregister(*hotkey) {
                tracing::error!(%error, "Native shortcut unregistration failed");
            }
        }
        if let Ok(mut routes) = self.routes.lock() {
            routes.clear();
        }
    }
}

#[derive(Clone, Debug)]
pub struct Shortcut {
    pub id: Arc<str>,
    pub accelerator: Arc<str>,
    pub active_during_capture_only: bool,
}

pub trait ShortcutAdapter {
    fn replace(&mut self, bindings: &[Shortcut]) -> Result<()>;
}

pub struct Shortcuts<A> {
    adapter: A,
    configured: Vec<Shortcut>,
}

impl<A: ShortcutAdapter> Shortcuts<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            configured: Vec::new(),
        }
    }

    pub fn configure(&mut self, bindings: Vec<Shortcut>, capturing: bool) -> Result<()> {
        let mut ids = HashSet::new();
        let mut accelerators = HashSet::new();
        if bindings.len() > 32
            || bindings.iter().any(|binding| {
                binding.id.is_empty()
                    || binding.accelerator.is_empty()
                    || !ids.insert(binding.id.clone())
                    || !accelerators.insert(binding.accelerator.clone())
            })
        {
            return Err(ServiceError::Failed(
                "Invalid or duplicate global shortcuts".into(),
            ));
        }
        let active: Vec<_> = bindings
            .iter()
            .filter(|binding| capturing || !binding.active_during_capture_only)
            .cloned()
            .collect();
        self.adapter.replace(&active)?;
        self.configured = bindings;
        Ok(())
    }

    pub fn set_capturing(&mut self, capturing: bool) -> Result<()> {
        self.configure(self.configured.clone(), capturing)
    }
}

pub struct UnavailableShortcuts;

impl ShortcutAdapter for UnavailableShortcuts {
    fn replace(&mut self, _: &[Shortcut]) -> Result<()> {
        Err(ServiceError::Unsupported("Native global shortcuts require registration and a Wayland portal adapter where applicable".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Host(Vec<Shortcut>);
    impl ShortcutAdapter for Host {
        fn replace(&mut self, bindings: &[Shortcut]) -> Result<()> {
            self.0 = bindings.to_vec();
            Ok(())
        }
    }

    #[test]
    fn capture_cancellation_binding_follows_activity_and_invalid_updates_preserve_config() {
        let mut shortcuts = Shortcuts::new(Host::default());
        let bindings = vec![
            Shortcut {
                id: "start".into(),
                accelerator: "Alt+KeyN".into(),
                active_during_capture_only: false,
            },
            Shortcut {
                id: "cancel".into(),
                accelerator: "Escape".into(),
                active_during_capture_only: true,
            },
        ];
        for binding in &bindings {
            assert!(HotKey::from_str(&binding.accelerator).is_ok());
        }
        shortcuts.configure(bindings.clone(), false).unwrap();
        assert_eq!(shortcuts.adapter.0.len(), 1);
        shortcuts.set_capturing(true).unwrap();
        assert_eq!(shortcuts.adapter.0.len(), 2);
        assert!(
            shortcuts
                .configure(vec![bindings[0].clone(), bindings[0].clone()], true)
                .is_err()
        );
        assert_eq!(shortcuts.configured.len(), 2);
        shortcuts.set_capturing(false).unwrap();
        assert_eq!(shortcuts.adapter.0.len(), 1);
    }
}
