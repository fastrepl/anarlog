use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
};

use desktop_runtime::Result;
use gpui::{App, Application, KeyBinding, Keystroke, Menu, MenuItem, OsAction, WindowHandle};

use crate::{
    application::ApplicationView,
    platform::{
        deeplinks::NativeDeepLinks,
        shortcuts::{NativeShortcuts, Shortcut, ShortcutAdapter},
        tray::{NativeTray, TrayAction, TrayAdapter, TrayModel, UpdateMenu},
    },
    zoom,
};

gpui::actions!(
    native,
    [
        About,
        CheckUpdate,
        Settings,
        HideApp,
        HideOthers,
        ShowAll,
        Quit,
        NewNote,
        CloseWindow,
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll,
        ActualSize,
        ZoomIn,
        ZoomOut,
        ToggleFullScreen,
        Minimize,
        Zoom,
        ReportBug,
        SuggestFeature,
    ]
);

/// Key equivalents shown in the menu bar. No view uses this context, so GPUI never matches
/// them: the focused view handles the keys itself and the menu only acts on unhandled ones.
const MENU_ONLY: &str = "AnarlogAppMenu";
const FEEDBACK_URL: &str = "https://anarlog.so/discord";

pub enum NativeEvent {
    Action(TrayAction),
    NewNote,
    About,
    /// The app menu's Quit: like shipping, it closes (hides) the main window.
    CloseMain,
    /// A confirmed quit: drains and exits.
    Quit,
    DeepLink {
        id: u64,
        raw: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuState {
    /// The update item's label and whether it is enabled; download progress never rebuilds menus.
    pub update: (&'static str, bool),
    pub fullscreen: bool,
}

/// Shipping's macOS app menu (`plugins/tray/src/ext.rs` `build_app_menu`).
pub fn app_menus(state: &MenuState) -> Vec<Menu> {
    let menu = |name: &str, items: Vec<MenuItem>| Menu {
        name: name.to_string().into(),
        items,
        disabled: false,
    };
    vec![
        menu(
            "Anarlog",
            vec![
                MenuItem::action("About Anarlog", About),
                MenuItem::action(state.update.0, CheckUpdate).disabled(!state.update.1),
                MenuItem::action("Settings", Settings),
                MenuItem::separator(),
                MenuItem::os_submenu("Services", gpui::SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Hide Anarlog", HideApp),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ],
        ),
        menu(
            "File",
            vec![
                MenuItem::action("New Note", NewNote),
                MenuItem::action("Close Window", CloseWindow),
            ],
        ),
        menu(
            "Edit",
            vec![
                MenuItem::os_action("Undo", Undo, OsAction::Undo),
                MenuItem::os_action("Redo", Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            ],
        ),
        menu(
            "View",
            vec![
                #[cfg(not(target_os = "windows"))]
                MenuItem::action("Actual Size", ActualSize),
                #[cfg(not(target_os = "windows"))]
                MenuItem::action("Zoom In", ZoomIn),
                #[cfg(not(target_os = "windows"))]
                MenuItem::action("Zoom Out", ZoomOut),
                #[cfg(not(target_os = "windows"))]
                MenuItem::separator(),
                MenuItem::action(
                    if state.fullscreen {
                        "Exit Full Screen"
                    } else {
                        "Enter Full Screen"
                    },
                    ToggleFullScreen,
                ),
            ],
        ),
        menu(
            "Window",
            vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
                MenuItem::separator(),
                MenuItem::action("Close Window", CloseWindow),
            ],
        ),
        menu(
            "Help",
            vec![
                MenuItem::action("Report Bug", ReportBug),
                MenuItem::action("Suggest Feature", SuggestFeature),
            ],
        ),
    ]
}

fn menu_bindings() -> Vec<KeyBinding> {
    let only = Some(MENU_ONLY);
    vec![
        KeyBinding::new("cmd-q", Quit, only),
        KeyBinding::new("cmd-h", HideApp, only),
        KeyBinding::new("alt-cmd-h", HideOthers, only),
        KeyBinding::new("cmd-n", NewNote, only),
        KeyBinding::new("cmd-w", CloseWindow, only),
        KeyBinding::new("cmd-z", Undo, only),
        KeyBinding::new("cmd-shift-z", Redo, only),
        KeyBinding::new("cmd-x", Cut, only),
        KeyBinding::new("cmd-c", Copy, only),
        KeyBinding::new("cmd-v", Paste, only),
        KeyBinding::new("cmd-a", SelectAll, only),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, only),
        KeyBinding::new("cmd-m", Minimize, only),
    ]
}

/// Shipping's zoom keys work in every window and also take Control. Unlike the menu-only keys,
/// GPUI matches these itself; the menu shows the first binding of each action.
#[cfg(not(target_os = "windows"))]
fn zoom_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("ctrl-+", ZoomIn, None),
        KeyBinding::new("ctrl-=", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-_", ZoomOut, None),
        KeyBinding::new("ctrl--", ZoomOut, None),
        KeyBinding::new("ctrl-_", ZoomOut, None),
        KeyBinding::new("cmd-0", ActualSize, None),
        KeyBinding::new("ctrl-0", ActualSize, None),
    ]
}

/// Runs a window command on the key window once the current dispatch has released it.
fn on_active_window(cx: &mut App, command: impl FnOnce(&mut gpui::Window, &mut App) + 'static) {
    cx.defer(move |cx| {
        if let Some(window) = cx.active_window() {
            let _ = window.update(cx, |_, window, cx| command(window, cx));
        }
    });
}

/// The text views own their edit keys; menu clicks replay the shortcut into the focused view.
fn replay(cx: &mut App, keystroke: &'static str) {
    on_active_window(cx, move |window, cx| {
        if let Ok(keystroke) = Keystroke::parse(keystroke) {
            window.dispatch_keystroke(keystroke, cx);
        }
    });
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
    menus: Option<MenuState>,
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

    fn send(&self, event: NativeEvent) {
        if self.sender.try_send(event).is_err() {
            tracing::warn!("Native event queue is full");
        }
    }

    pub fn attach(&self, cx: &mut App) -> (NativeHandles, Vec<String>) {
        let mut errors = Vec::new();
        let tray = match NativeTray::new() {
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
        self.install_menu_actions(cx);
        #[cfg(target_os = "macos")]
        self.install_quit_keys(cx);
        let menus = MenuState {
            update: (UpdateMenu::default().label(), true),
            fullscreen: false,
        };
        cx.set_menus(app_menus(&menus));
        (
            NativeHandles {
                tray,
                _shortcuts: shortcuts,
                menus: Some(menus),
            },
            errors,
        )
    }

    fn install_menu_actions(&self, cx: &mut App) {
        let events = self.clone();
        cx.on_action(move |_: &About, _| events.send(NativeEvent::About));
        let events = self.clone();
        cx.on_action(move |_: &CheckUpdate, _| {
            events.send(NativeEvent::Action(TrayAction::CheckUpdate))
        });
        let events = self.clone();
        cx.on_action(move |_: &Settings, _| events.send(NativeEvent::Action(TrayAction::Settings)));
        let events = self.clone();
        cx.on_action(move |_: &Quit, _| events.send(NativeEvent::CloseMain));
        let events = self.clone();
        cx.on_action(move |_: &NewNote, _| events.send(NativeEvent::NewNote));
        cx.on_action(|_: &HideApp, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.on_action(|_: &CloseWindow, cx| {
            on_active_window(cx, |window, cx| {
                crate::platform::windows::perform_close(window, cx)
            })
        });
        cx.on_action(|_: &ActualSize, cx| zoom::apply(zoom::Step::Reset, cx));
        cx.on_action(|_: &ZoomIn, cx| zoom::apply(zoom::Step::In, cx));
        cx.on_action(|_: &ZoomOut, cx| zoom::apply(zoom::Step::Out, cx));
        cx.on_action(|_: &ToggleFullScreen, cx| {
            on_active_window(cx, |window, _| window.toggle_fullscreen())
        });
        cx.on_action(|_: &Minimize, cx| on_active_window(cx, |window, _| window.minimize_window()));
        cx.on_action(|_: &Zoom, cx| on_active_window(cx, |window, _| window.zoom_window()));
        cx.on_action(|_: &Undo, cx| replay(cx, "cmd-z"));
        cx.on_action(|_: &Redo, cx| replay(cx, "cmd-shift-z"));
        cx.on_action(|_: &Cut, cx| replay(cx, "cmd-x"));
        cx.on_action(|_: &Copy, cx| replay(cx, "cmd-c"));
        cx.on_action(|_: &Paste, cx| replay(cx, "cmd-v"));
        cx.on_action(|_: &SelectAll, cx| replay(cx, "cmd-a"));
        cx.on_action(|_: &ReportBug, cx| cx.open_url(FEEDBACK_URL));
        cx.on_action(|_: &SuggestFeature, cx| cx.open_url(FEEDBACK_URL));
        cx.bind_keys(menu_bindings());
        #[cfg(not(target_os = "windows"))]
        cx.bind_keys(zoom_bindings());
    }

    /// Shipping's ⌘Q interceptor (`crates/intercept`): the first press shows "Press ⌘Q again
    /// to Quit", a second press (or ⌘⇧Q) quits through the drain.
    #[cfg(target_os = "macos")]
    fn install_quit_keys(&self, cx: &mut App) {
        use crate::application::lifecycle::{QUIT_OVERLAY_DURATION, QuitOverlay, QuitSignal};
        use futures::StreamExt;
        let keys = Rc::new(RefCell::new(
            crate::application::lifecycle::QuitKeys::default(),
        ));
        let (signals, mut received) = futures::channel::mpsc::unbounded();
        quit_monitor::install(keys.clone(), signals.clone());
        let events = self.clone();
        cx.spawn(async move |cx| {
            let mut overlay = QuitOverlay::default();
            while let Some(signal) = received.next().await {
                match signal {
                    QuitSignal::Released(generation) => {
                        let (keys, signals) = (keys.clone(), signals.clone());
                        cx.spawn(async move |cx| {
                            cx.background_executor().timer(QUIT_OVERLAY_DURATION).await;
                            if keys
                                .try_borrow_mut()
                                .is_ok_and(|mut keys| keys.timeout(generation))
                            {
                                let _ = signals.unbounded_send(QuitSignal::Hide);
                            }
                        })
                        .detach();
                    }
                    QuitSignal::Show => cx.update(|cx| overlay.show(cx)),
                    QuitSignal::Hide => cx.update(|cx| overlay.hide(cx)),
                    QuitSignal::Quit => {
                        cx.update(|cx| overlay.hide(cx));
                        events.send(NativeEvent::Quit);
                    }
                }
            }
        })
        .detach();
    }

    /// Re-applies the app menu when its update item or the full-screen title changes.
    pub fn sync_menus(&self, handles: &mut NativeHandles, state: MenuState, cx: &mut App) {
        if handles.menus.as_ref() != Some(&state) {
            cx.set_menus(app_menus(&state));
            handles.menus = Some(state);
        }
    }

    pub fn poll(
        &self,
        handles: &mut NativeHandles,
        model: &TrayModel,
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
        if let Some(tray) = &mut handles.tray {
            if let Err(error) = tray.update(model) {
                tracing::warn!(%error, "tray update failed");
            }
            events.extend(tray.poll().into_iter().map(NativeEvent::Action));
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

/// An AppKit local event monitor, like shipping's `QuitInterceptor.swift`: it sees ⌘Q in every
/// window before the menu's key equivalent, including key repeats and releases.
#[cfg(target_os = "macos")]
mod quit_monitor {
    use std::{cell::RefCell, ffi::CStr, rc::Rc};

    use futures::channel::mpsc::UnboundedSender;
    use objc2::{
        class, msg_send,
        runtime::{AnyObject, Bool},
    };

    use crate::application::lifecycle::{QuitKeys, QuitSignal};

    const KEY_DOWN: usize = 10;
    const KEY_UP: usize = 11;
    const FLAGS_CHANGED: usize = 12;
    const SHIFT: usize = 1 << 17;
    const COMMAND: usize = 1 << 20;
    const DEVICE_INDEPENDENT: usize = 0xffff_0000;

    /// SAFETY: `event` is a live NSEvent of a key type.
    unsafe fn is_q(event: *mut AnyObject) -> bool {
        unsafe {
            let characters: *mut AnyObject = msg_send![event, charactersIgnoringModifiers];
            if characters.is_null() {
                return false;
            }
            let text: *const std::ffi::c_char = msg_send![characters, UTF8String];
            !text.is_null() && CStr::from_ptr(text).to_bytes().eq_ignore_ascii_case(b"q")
        }
    }

    pub fn install(keys: Rc<RefCell<QuitKeys>>, signals: UnboundedSender<QuitSignal>) {
        let send = move |signal: Option<QuitSignal>| {
            if let Some(signal) = signal {
                let _ = signals.unbounded_send(signal);
            }
        };
        let handler = block2::RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
            if event.is_null() {
                return event;
            }
            // SAFETY: AppKit passes a live NSEvent matching the monitor's mask.
            unsafe {
                let kind: usize = msg_send![event, type];
                let flags: usize = msg_send![event, modifierFlags];
                let flags = flags & DEVICE_INDEPENDENT;
                let Ok(mut keys) = keys.try_borrow_mut() else {
                    return event;
                };
                match kind {
                    KEY_DOWN if flags & COMMAND != 0 && is_q(event) => {
                        let repeat: Bool = msg_send![event, isARepeat];
                        send(keys.press(flags & SHIFT != 0, repeat.as_bool()));
                        std::ptr::null_mut()
                    }
                    KEY_UP if is_q(event) => {
                        send(keys.release().map(QuitSignal::Released));
                        event
                    }
                    FLAGS_CHANGED if flags & COMMAND == 0 => {
                        send(keys.release().map(QuitSignal::Released));
                        event
                    }
                    _ => event,
                }
            }
        });
        let mask: u64 = (1 << KEY_DOWN) | (1 << KEY_UP) | (1 << FLAGS_CHANGED);
        // SAFETY: AppKit copies the block and keeps the monitor for the process lifetime.
        let monitor: *mut AnyObject = unsafe {
            msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: mask, handler: &*handler]
        };
        if monitor.is_null() {
            tracing::warn!("⌘Q monitor is unavailable; the app menu handles Quit");
        }
    }
}
