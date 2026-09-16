use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use tauri::Manager;
use tauri_plugin_global_shortcut::{Modifiers, Shortcut};
use tauri_specta::Event;

use crate::ShortcutEvent;

pub struct Registration {
    stop: tokio::sync::oneshot::Sender<()>,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl Registration {
    pub async fn close(self) {
        let _ = self.stop.send(());
        let _ = self.task.await;
    }
}

pub async fn register(app: tauri::AppHandle, key: Shortcut) -> Result<Registration, String> {
    let portal = GlobalShortcuts::new().await.map_err(|e| {
        format!("This Wayland desktop does not provide the Global Shortcuts portal: {e}")
    })?;
    let session = portal
        .create_session(Default::default())
        .await
        .map_err(|e| e.to_string())?;
    let setup = tokio::time::timeout(std::time::Duration::from_secs(45), async {
        let pressed = portal
            .receive_activated()
            .await
            .map_err(|e| e.to_string())?;
        let released = portal
            .receive_deactivated()
            .await
            .map_err(|e| e.to_string())?;
        let trigger = portal_trigger(key);
        portal
            .bind_shortcuts(
                &session,
                &[
                    NewShortcut::new("dictate", "Dictate with Anarlog")
                        .preferred_trigger(trigger.as_str()),
                    NewShortcut::new("cancel-dictation", "Cancel Anarlog dictation")
                        .preferred_trigger("CTRL+ALT+Escape"),
                ],
                None,
                Default::default(),
            )
            .await
            .and_then(|request| request.response())
            .map_err(|e| e.to_string())?;
        Ok::<_, String>((pressed, released))
    })
    .await;
    let (mut pressed, mut released) = match setup {
        Ok(Ok(streams)) => streams,
        error => {
            let _ = session.close().await;
            return Err(match error {
                Ok(Err(error)) => error,
                _ => "Shortcut setup timed out. Retry setup and approve your desktop's shortcut prompt.".into(),
            });
        }
    };
    let session_path: String =
        serde_json::from_value(serde_json::to_value(&session).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let (stop, mut stopped) = tokio::sync::oneshot::channel();

    let task = tauri::async_runtime::spawn(async move {
        let Ok(mut closed) = session.receive_closed().await else {
            let _ = session.close().await;
            let _ = ShortcutEvent::Cancelled.emit(&app);
            return;
        };
        let mut down = false;
        loop {
            tokio::select! {
                _ = &mut stopped => break,
                _ = closed.next() => break,
                event = pressed.next() => {
                    let Some(event) = event else { break };
                    if event.session_handle().as_str() != session_path { continue; }
                    match event.shortcut_id() {
                        "dictate" if !down => { down = true; let _ = ShortcutEvent::Pressed.emit(&app); }
                        "cancel-dictation" if app.state::<super::GlobalState>().active.load(std::sync::atomic::Ordering::SeqCst) => {
                            let _ = ShortcutEvent::Cancelled.emit(&app);
                        }
                        _ => {}
                    }
                }
                event = released.next() => {
                    let Some(event) = event else { break };
                    if event.session_handle().as_str() == session_path && event.shortcut_id() == "dictate" {
                        down = false;
                        let _ = ShortcutEvent::Released.emit(&app);
                    }
                }
            }
        }
        let _ = session.close().await;
        let _ = ShortcutEvent::Cancelled.emit(&app);
    });
    Ok(Registration { stop, task })
}

fn portal_trigger(key: Shortcut) -> String {
    let mut parts = Vec::new();
    for (flag, label) in [
        (Modifiers::CONTROL, "CTRL"),
        (Modifiers::ALT, "ALT"),
        (Modifiers::SHIFT, "SHIFT"),
        (Modifiers::SUPER, "LOGO"),
    ] {
        if key.mods.contains(flag) {
            parts.push(label.to_string());
        }
    }
    let code = key.key.to_string();
    parts.push(match code.as_str() {
        "Space" => "space".into(),
        "Backspace" => "BackSpace".into(),
        _ => code
            .strip_prefix("Key")
            .or_else(|| code.strip_prefix("Digit"))
            .unwrap_or(&code)
            .into(),
    });
    parts.join("+")
}
