use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_specta::Event;

use crate::{ShortcutEvent, ShortcutPluginExt};

pub fn uses_portal() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[derive(Default)]
pub struct GlobalState {
    registration: tokio::sync::Mutex<Registration>,
    active: Arc<AtomicBool>,
}

#[derive(Default)]
struct Registration {
    shortcut: Option<Shortcut>,
    cancel_keys: Vec<Shortcut>,
    #[cfg(target_os = "linux")]
    portal: Option<portal::Registration>,
}

#[tauri::command]
#[specta::specta]
pub async fn configure(app: tauri::AppHandle, shortcut: Option<String>) -> Result<(), String> {
    let state = app.state::<GlobalState>();
    let mut registration = state.registration.lock().await;
    state.active.store(false, Ordering::SeqCst);
    if !uses_portal() {
        let Some(global) =
            app.try_state::<tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>>()
        else {
            return if shortcut.is_none() {
                Ok(())
            } else {
                Err("Global shortcuts are unavailable in this desktop session.".into())
            };
        };
        while let Some(key) = registration.cancel_keys.last().copied() {
            global.unregister(key).map_err(|e| e.to_string())?;
            registration.cancel_keys.pop();
        }
    }
    app.shortcut().unregister().map_err(|e| e.to_string())?;
    if let Some(previous) = registration.shortcut {
        app.global_shortcut()
            .unregister(previous)
            .map_err(|e| e.to_string())?;
        registration.shortcut = None;
    }
    #[cfg(target_os = "linux")]
    if let Some(portal) = registration.portal.take() {
        portal.close().await;
    }
    let Some(shortcut) = shortcut else {
        return Ok(());
    };

    #[cfg(target_os = "macos")]
    if shortcut == "Fn" || shortcut == "RightCommand" {
        return app
            .shortcut()
            .register(
                crate::HotKey {
                    key: None,
                    modifiers: vec![if shortcut == "Fn" {
                        crate::Modifier::Fn
                    } else {
                        crate::Modifier::RightCommand
                    }],
                },
                crate::Options {
                    double_tap_lock_enabled: false,
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string());
    }

    let key: Shortcut = shortcut
        .parse()
        .map_err(|e| format!("Invalid dictation shortcut: {e}"))?;
    if key.mods.is_empty() {
        return Err("Choose a shortcut with Control, Alt, Shift, or Command.".into());
    }
    if key.key == Code::Escape {
        return Err("Escape is reserved for cancelling dictation. Choose another key.".into());
    }
    #[cfg(target_os = "linux")]
    if uses_portal() {
        registration.portal = Some(portal::register(app.clone(), key).await?);
        return Ok(());
    }

    let down = Arc::new(AtomicBool::new(false));
    app.global_shortcut()
        .on_shortcut(key, move |app, _, event| {
            let pressed = event.state == ShortcutState::Pressed;
            if down.swap(pressed, Ordering::SeqCst) == pressed {
                return;
            }
            let event = if pressed {
                ShortcutEvent::Pressed
            } else {
                ShortcutEvent::Released
            };
            let _ = event.emit(app);
        })
        .map_err(|e| {
            format!("Could not register dictation shortcut. It may be used by another app: {e}")
        })?;
    registration.shortcut = Some(key);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_active(app: tauri::AppHandle, active: bool) -> Result<(), String> {
    let state = app.state::<GlobalState>();
    state.active.store(active, Ordering::SeqCst);
    #[cfg(target_os = "linux")]
    if uses_portal() {
        return Ok(());
    }
    if app
        .try_state::<tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>>()
        .is_none()
    {
        return Ok(());
    }
    if active {
        let mut registration = state.registration.lock().await;
        let mut keys = vec![Shortcut::new(None, Code::Escape)];
        if let Some(shortcut) = registration.shortcut {
            keys.push(Shortcut::new(Some(shortcut.mods), Code::Escape));
        }
        for key in keys {
            if registration.cancel_keys.contains(&key) {
                continue;
            }
            let enabled = state.active.clone();
            app.global_shortcut()
                .on_shortcut(key, move |app, _, event| {
                    if event.state == ShortcutState::Pressed && enabled.load(Ordering::SeqCst) {
                        let _ = ShortcutEvent::Cancelled.emit(app);
                    }
                })
                .map_err(|e| e.to_string())?;
            registration.cancel_keys.push(key);
        }
    } else {
        let mut registration = state.registration.lock().await;
        while let Some(key) = registration.cancel_keys.last().copied() {
            app.global_shortcut()
                .unregister(key)
                .map_err(|e| e.to_string())?;
            registration.cancel_keys.pop();
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
mod portal;
