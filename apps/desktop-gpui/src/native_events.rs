use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
};

use desktop_runtime::{Result, ServiceError};
use gpui::{App, Application, Menu, MenuItem, WindowHandle};

use crate::{
    application::ApplicationView,
    platform::{
        deeplinks::NativeDeepLinks,
        shortcuts::{NativeShortcuts, Shortcut, ShortcutAdapter},
        tray::{NativeTray, TrayAction, TrayState},
    },
};

gpui::actions!(native, [Quit, NewNote, Record, Settings, Show, CheckUpdate]);

pub enum NativeEvent {
    Action(TrayAction),
    NewNote,
    DeepLink { id: u64, raw: String },
}

#[derive(Clone)]
pub struct NativeEvents {
    sender: mpsc::SyncSender<NativeEvent>,
    receiver: Arc<Mutex<mpsc::Receiver<NativeEvent>>>,
    links: NativeDeepLinks,
}

pub struct NativeHandles {
    pub tray: Option<NativeTray>,
    _shortcuts: Option<Rc<RefCell<NativeShortcuts>>>,
}

impl NativeEvents {
    pub fn install(application: &Application) -> Self {
        let (sender, receiver) = mpsc::sync_channel(64);
        let events = Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            links: NativeDeepLinks::default(),
        };
        let links = events.clone();
        application.on_open_urls(move |urls| {
            for url in urls.into_iter().take(64) {
                match links.links.receive(&url) {
                    Ok(id) => {
                        if links
                            .sender
                            .try_send(NativeEvent::DeepLink { id, raw: url })
                            .is_err()
                        {
                            let _ = links.links.acknowledge(id);
                            tracing::warn!("Native event queue is full");
                        }
                    }
                    Err(error) => tracing::warn!("Deep link rejected: {error}"),
                }
            }
        });
        let reopen = events.clone();
        application.on_reopen(move |_| {
            let _ = reopen
                .sender
                .try_send(NativeEvent::Action(TrayAction::Open));
        });
        events
    }

    pub fn next(&self) -> Option<NativeEvent> {
        self.receiver.lock().ok()?.try_recv().ok()
    }

    pub fn attach(&self, cx: &mut App) -> (NativeHandles, Vec<String>) {
        let mut errors = Vec::new();
        let icon = image::load_from_memory(include_bytes!(
            "../../desktop/src-tauri/icons/dev/32x32.png"
        ))
        .map(|image| image.into_rgba8())
        .map_err(|error| ServiceError::Failed(error.to_string().into()))
        .and_then(|image| {
            tray_icon::Icon::from_rgba(image.to_vec(), image.width(), image.height())
                .map_err(|error| ServiceError::Failed(error.to_string().into()))
        });
        let tray = match icon.and_then(NativeTray::new) {
            Ok(tray) => Some(tray),
            Err(error) => {
                errors.push(format!("Tray: {error}"));
                None
            }
        };
        let sender = self.sender.clone();
        let (dictation, dictation_events) = crate::dictation::shortcut::channel();
        let dictation_route = crate::dictation::shortcut::router(dictation.clone());
        let base = vec![
            Shortcut {
                id: "show".into(),
                accelerator: "CommandOrControl+Shift+KeyH".into(),
                active_during_capture_only: false,
            },
            Shortcut {
                id: "record".into(),
                accelerator: "CommandOrControl+Shift+KeyR".into(),
                active_during_capture_only: false,
            },
        ];
        let shortcuts: Result<NativeShortcuts> = NativeShortcuts::new(move |id, pressed| {
            if dictation_route(&id, pressed) {
                return;
            }
            if pressed {
                let action = match id.as_ref() {
                    "show" => NativeEvent::Action(TrayAction::Open),
                    "record" => NativeEvent::Action(TrayAction::StartMeeting),
                    _ => return,
                };
                let _ = sender.try_send(action);
            }
        })
        .and_then(|mut shortcuts| {
            shortcuts.replace(&base)?;
            Ok(shortcuts)
        });
        let shortcuts = match shortcuts {
            Ok(shortcuts) => {
                // Dictation adds its own keys to the same registration and event handler.
                let shortcuts = Rc::new(RefCell::new(shortcuts));
                cx.set_global(crate::dictation::shortcut::NativeHotkeys {
                    shortcuts: shortcuts.clone(),
                    base,
                    sender: dictation,
                    events: dictation_events,
                });
                Some(shortcuts)
            }
            Err(error) => {
                errors.push(format!("Global shortcuts: {error}"));
                None
            }
        };
        let sender = self.sender.clone();
        cx.on_action(move |_: &Quit, _| {
            let _ = sender.try_send(NativeEvent::Action(TrayAction::Quit));
        });
        let sender = self.sender.clone();
        cx.on_action(move |_: &Settings, _| {
            let _ = sender.try_send(NativeEvent::Action(TrayAction::Settings));
        });
        let sender = self.sender.clone();
        cx.on_action(move |_: &NewNote, _| {
            let _ = sender.try_send(NativeEvent::NewNote);
        });
        let sender = self.sender.clone();
        cx.on_action(move |_: &Record, _| {
            let _ = sender.try_send(NativeEvent::Action(TrayAction::StartMeeting));
        });
        let sender = self.sender.clone();
        cx.on_action(move |_: &Show, _| {
            let _ = sender.try_send(NativeEvent::Action(TrayAction::Open));
        });
        let sender = self.sender.clone();
        cx.on_action(move |_: &CheckUpdate, _| {
            let _ = sender.try_send(NativeEvent::Action(TrayAction::InstallUpdate));
        });
        cx.set_menus(vec![
            Menu {
                name: "Anarlog".into(),
                items: vec![
                    MenuItem::action("Settings…", Settings),
                    MenuItem::action("Check for updates…", CheckUpdate),
                    MenuItem::separator(),
                    MenuItem::action("Quit Anarlog", Quit),
                ],
                disabled: false,
            },
            Menu {
                name: "File".into(),
                items: vec![
                    MenuItem::action("New note window", NewNote),
                    MenuItem::action("Start meeting", Record),
                ],
                disabled: false,
            },
            Menu {
                name: "Window".into(),
                items: vec![MenuItem::action("Show Anarlog", Show)],
                disabled: false,
            },
        ]);
        (
            NativeHandles {
                tray,
                _shortcuts: shortcuts,
            },
            errors,
        )
    }

    pub fn poll(
        &self,
        handles: &NativeHandles,
        state: &TrayState,
        main: WindowHandle<ApplicationView>,
        cx: &mut App,
    ) {
        if !main
            .update(cx, |view, _, _| view.native_ready())
            .unwrap_or(false)
        {
            return;
        }
        let mut events = Vec::new();
        if let Some(tray) = &handles.tray {
            events.extend(tray.poll(state).into_iter().map(NativeEvent::Action));
        }
        for _ in 0..64 {
            let Some(event) = self.next() else {
                break;
            };
            events.push(event);
        }
        for event in events {
            let id = match &event {
                NativeEvent::DeepLink { id, .. } => Some(*id),
                _ => None,
            };
            let _ = main.update(cx, |view, window, cx| view.native_event(event, window, cx));
            if let Some(id) = id {
                let _ = self.links.acknowledge(id);
            }
        }
    }
}
