//! The app lock and the reveal state of locked notes (`lock/{store,gate,screen,auth}.ts(x)`).
//! With "Lock app" on, the main window shows "Anarlog is Locked" until the device owner
//! authenticates, and locks again whenever the main window is hidden. Locking the app also
//! hides every note that was revealed.

use std::collections::HashSet;

use desktop_runtime::{QueryWatch, Reply, RuntimeHandle, ServiceError, SessionId};
use gpui::{
    App, ClickEvent, Div, Global, MouseButton, SharedString, Subscription, Task, Window,
    WindowVisibility, div, prelude::*, px, svg,
};
use serde_json::Value;

use crate::{
    meeting::{access, capture::Phase},
    platform::device_auth,
    product::settings,
    ui::theme::{MEDIUM, Theme, theme},
};

/// `DEVICE_AUTH_REASON` in `lock/auth.ts`.
pub const OPEN_APP: &str = "open";
pub const LOCK_NOTE: &str = "lock this note";
pub const UNLOCK_NOTE: &str = "unlock this note";

const LOCKED_NOTES_SQL: &str =
    "SELECT json_group_array(id) AS ids FROM sessions WHERE locked = 1 AND deleted_at IS NULL";

#[derive(Default)]
pub struct AppLock {
    runtime: Option<RuntimeHandle>,
    /// The `lock_app` setting; `None` until it was read.
    enabled: Option<bool>,
    available: bool,
    authenticating: bool,
    unlocked: bool,
    /// Bumped by every lock, so an authentication that started earlier cannot unlock.
    epoch: u64,
    revealed: HashSet<SessionId>,
    /// `None` until the first read.
    locked_notes: Option<HashSet<SessionId>>,
    /// The gate prompted for this launch already (shipping's `promptedRef`).
    prompted: bool,
    /// The main window is hidden; the next prompt waits until it shows again.
    hidden: bool,
}

impl Global for AppLock {}

impl AppLock {
    fn should_lock(&self) -> bool {
        self.enabled == Some(true) && self.available
    }
}

/// What a note shows: its content, the "Note is Locked" screen, or nothing until known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteAccess {
    Open,
    Locked,
    Unknown,
}

/// Installs the lock, reads the setting and the locked notes, and follows the main window.
pub fn install(runtime: &RuntimeHandle, window: &mut Window, cx: &mut App) -> Subscription {
    cx.set_global(AppLock {
        runtime: Some(runtime.clone()),
        available: device_auth::available(cx),
        hidden: presence::ordered_out(window),
        ..Default::default()
    });
    follow(settings::watch(runtime), cx, |rows, cx| {
        let enabled = match rows {
            Ok(rows) => Some(lock_setting(rows)),
            Err(error) => {
                tracing::warn!(%error, "the app lock setting is unavailable");
                None
            }
        };
        set_enabled(enabled, cx);
    });
    follow(
        runtime.watch_query(LOCKED_NOTES_SQL.into(), vec![]),
        cx,
        |rows, cx| match rows {
            Ok(rows) => set_locked_notes(locked_ids(rows), cx),
            Err(error) => tracing::warn!(%error, "locked notes are unavailable"),
        },
    );
    window.observe_window_visibility(|visibility, window, cx| match visibility {
        WindowVisibility::Hidden if presence::ordered_out(window) => main_hidden(cx),
        WindowVisibility::Visible if cx.try_global::<AppLock>().is_some_and(|lock| lock.hidden) => {
            main_shown(cx)
        }
        _ => {}
    })
}

/// `lock_app` resolved like every setting, from its own and the legacy rows only, so another
/// unreadable setting cannot turn the lock off. An unreadable lock setting keeps it on.
fn lock_setting(rows: &[Value]) -> bool {
    let rows = rows
        .iter()
        .filter(|row| {
            let id = row["id"].as_str().unwrap_or_default();
            id == "lock_app" || settings::definition(id).is_err()
        })
        .cloned()
        .collect::<Vec<_>>();
    settings::decode(&rows).map_or(true, |snapshot| {
        snapshot
            .get("lock_app")
            .and_then(|setting| setting.value.as_bool())
            .unwrap_or(false)
    })
}

fn locked_ids(rows: &[Value]) -> HashSet<SessionId> {
    rows.first()
        .and_then(|row| row["ids"].as_str())
        .and_then(|ids| serde_json::from_str::<Vec<String>>(ids).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|id| SessionId(id.into()))
        .collect()
}

/// Applies every snapshot of a watch until the runtime closes it.
fn follow(
    reply: desktop_runtime::Result<Reply<QueryWatch>>,
    cx: &mut App,
    apply: impl Fn(Result<&[Value], ServiceError>, &mut App) + 'static,
) {
    cx.spawn(async move |cx| {
        let mut watch = match async { reply?.receive().await }.await {
            Ok(watch) => watch,
            Err(error) => {
                cx.update(|cx| apply(Err(error), cx));
                return;
            }
        };
        loop {
            if let Some(error) = watch
                .terminal_error()
                .or_else(|| watch.errors.try_recv().ok())
            {
                cx.update(|cx| apply(Err(error), cx));
                break;
            }
            let snapshot = watch.snapshots.borrow_and_update().clone();
            if snapshot.sequence > 0 {
                cx.update(|cx| apply(Ok(&snapshot.rows), cx));
            }
            if watch.snapshots.changed().await.is_err() {
                break;
            }
        }
        let _ = watch.unsubscribe().await;
    })
    .detach();
}

fn update<R>(cx: &mut App, f: impl FnOnce(&mut AppLock) -> R) -> R {
    let result = f(cx.default_global::<AppLock>());
    cx.refresh_windows();
    result
}

fn read<R: Default>(cx: &App, f: impl FnOnce(&AppLock) -> R) -> R {
    cx.try_global::<AppLock>().map(f).unwrap_or_default()
}

pub fn runtime(cx: &App) -> Option<RuntimeHandle> {
    read(cx, |lock| lock.runtime.clone())
}

/// Device authentication can run, so locks are offered (`state.available`).
pub fn available(cx: &App) -> bool {
    read(cx, |lock| lock.available)
}

/// `refreshAvailability`: device authentication may have been set up since launch.
pub fn refresh_availability(cx: &mut App) -> bool {
    let available = device_auth::available(cx);
    update(cx, |lock| lock.available = available);
    prompt_if_due(cx);
    available
}

pub fn authenticating(cx: &App) -> bool {
    read(cx, |lock| lock.authenticating)
}

pub fn app_locked(cx: &App) -> bool {
    read(cx, |lock| lock.should_lock() && !lock.unlocked)
}

pub fn note_access(id: &SessionId, cx: &App) -> NoteAccess {
    let Some(lock) = cx.try_global::<AppLock>() else {
        return NoteAccess::Open;
    };
    match &lock.locked_notes {
        None => NoteAccess::Unknown,
        Some(locked) if locked.contains(id) && !lock.revealed.contains(id) => NoteAccess::Locked,
        Some(_) => NoteAccess::Open,
    }
}

/// `Some(revealed)` for a locked note.
pub fn note_locked(id: &SessionId, cx: &App) -> Option<bool> {
    let lock = cx.try_global::<AppLock>()?;
    lock.locked_notes
        .as_ref()
        .is_some_and(|locked| locked.contains(id))
        .then(|| lock.revealed.contains(id))
}

pub fn note_lock_known(cx: &App) -> bool {
    read(cx, |lock| lock.locked_notes.is_some())
}

/// One prompt at a time; `apply` sees the result before the gate decides whether to prompt again.
fn authenticate_then(
    reason: &'static str,
    cx: &mut App,
    apply: impl FnOnce(&mut AppLock, bool) -> bool + 'static,
) -> Task<bool> {
    if read(cx, |lock| lock.authenticating) {
        return Task::ready(false);
    }
    update(cx, |lock| lock.authenticating = true);
    let reply = device_auth::authenticate(reason, cx);
    cx.spawn(async move |cx| {
        let authenticated = reply.await.unwrap_or(false);
        cx.update(|cx| {
            let result = update(cx, |lock| {
                lock.authenticating = false;
                apply(lock, authenticated)
            });
            prompt_if_due(cx);
            result
        })
    })
}

pub fn authenticate(reason: &'static str, cx: &mut App) -> Task<bool> {
    authenticate_then(reason, cx, |_, authenticated| authenticated)
}

pub fn unlock_app(cx: &mut App) -> Task<bool> {
    let epoch = read(cx, |lock| lock.epoch);
    authenticate_then(OPEN_APP, cx, move |lock, authenticated| {
        let unlocked = authenticated && lock.epoch == epoch;
        lock.unlocked |= unlocked;
        unlocked
    })
}

pub fn lock_app(cx: &mut App) {
    let capturing = capturing(cx);
    update(cx, |lock| {
        lock.epoch += 1;
        lock.unlocked = false;
        for id in lock.revealed.drain() {
            close_access(&id, capturing.as_ref());
        }
    });
}

/// Shipping's capture keeps writing when its note locks, so that note stays open to meeting
/// work until the capture ends.
fn capturing(cx: &App) -> Option<SessionId> {
    let update = cx
        .try_global::<crate::meeting::MeetingServices>()?
        .capture
        .take_update(u64::MAX);
    if matches!(
        update.phase,
        Phase::Loading | Phase::Listening | Phase::Finalizing
    ) {
        update.session
    } else {
        None
    }
}

fn close_access(id: &SessionId, capturing: Option<&SessionId>) {
    if capturing != Some(id) {
        access::close(&id.0);
    }
}

/// A finished capture's note closes to meeting work unless it is still revealed.
pub fn capture_ended(id: &SessionId, cx: &App) {
    if !read(cx, |lock| lock.revealed.contains(id)) {
        access::close(&id.0);
    }
}

/// `revealLockedNote`: shows a locked note until the app locks again.
pub fn reveal_note(id: SessionId, cx: &mut App) -> Task<bool> {
    if read(cx, |lock| lock.revealed.contains(&id)) {
        return Task::ready(true);
    }
    let epoch = read(cx, |lock| lock.epoch);
    authenticate_then(OPEN_APP, cx, move |lock, authenticated| {
        let revealed = authenticated && lock.epoch == epoch;
        if revealed {
            access::open(&id.0);
            lock.revealed.insert(id);
        }
        revealed
    })
}

/// Records a lock change before the watch reports it, so the note reacts at once.
pub fn note_lock_changed(id: SessionId, locked: bool, cx: &mut App) {
    let capturing = capturing(cx);
    update(cx, |lock| {
        let notes = lock.locked_notes.get_or_insert_default();
        if locked {
            notes.insert(id.clone());
            lock.revealed.remove(&id);
            close_access(&id, capturing.as_ref());
        } else {
            notes.remove(&id);
            access::open(&id.0);
            lock.revealed.insert(id);
        }
    });
}

fn set_enabled(enabled: Option<bool>, cx: &mut App) {
    update(cx, |lock| {
        // A setting that was never readable leaves the app open; there is no content to protect
        // when the library cannot be read either.
        lock.enabled = enabled.or(lock.enabled).or(Some(false));
    });
    prompt_if_due(cx);
}

fn set_locked_notes(locked: HashSet<SessionId>, cx: &mut App) {
    update(cx, |lock| lock.locked_notes = Some(locked));
}

/// The gate's first prompt: once per launch, and only while the main window shows.
fn prompt_if_due(cx: &mut App) {
    let due = read(cx, |lock| {
        lock.should_lock()
            && !lock.unlocked
            && !lock.authenticating
            && !lock.prompted
            && !lock.hidden
    });
    if due {
        update(cx, |lock| lock.prompted = true);
        unlock_app(cx).detach();
    }
}

/// Closing or hiding the main window locks the app without prompting.
pub fn main_hidden(cx: &mut App) {
    let should_lock = update(cx, |lock| {
        lock.hidden = true;
        if lock.should_lock() {
            lock.prompted = true;
        }
        lock.should_lock()
    });
    if should_lock {
        lock_app(cx);
    }
}

/// Showing the main window again prompts at once.
pub fn main_shown(cx: &mut App) {
    let prompt = update(cx, |lock| {
        lock.hidden = false;
        if !lock.should_lock() || lock.unlocked {
            return false;
        }
        // A prompt that the hide invalidated is still open; prompt again once it settles.
        lock.prompted = !lock.authenticating;
        lock.prompted
    });
    if prompt {
        unlock_app(cx).detach();
    }
}

/// Replaces the main window's content while locked, or while the setting is still unknown.
pub fn gate(window: &Window, cx: &App) -> Option<Div> {
    let lock = cx.try_global::<AppLock>()?;
    let colors = theme(window);
    let root = div()
        .size_full()
        .bg(colors.background)
        .text_color(colors.foreground);
    if lock.available && lock.enabled.is_none() {
        return Some(root);
    }
    app_locked(cx).then(|| {
        root.child(screen(
            "app-lock",
            "Anarlog is Locked",
            "View Anarlog",
            lock.authenticating,
            colors,
            |_, _, cx| {
                update(cx, |lock| lock.prompted = true);
                unlock_app(cx).detach();
            },
        ))
    })
}

/// `useDeviceAuthHint`.
pub fn hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "Use Touch ID or enter your password to view."
    } else if cfg!(target_os = "windows") {
        "Use Windows Hello face, PIN, or password to view."
    } else {
        "Authenticate to continue."
    }
}

/// `LockScreen`: the lock mark, title, hint and unlock button, centered.
pub fn screen(
    id: &'static str,
    title: impl Into<SharedString>,
    action: &'static str,
    authenticating: bool,
    colors: Theme,
    on_unlock: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
        .child(
            div()
                .max_w(px(384.))
                .px_6()
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .child(
                    div()
                        .size(px(56.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .bg(colors.muted)
                        .child(
                            svg()
                                .path("editor/LockPasswordIcon.svg")
                                .size(px(24.))
                                .text_color(colors.foreground),
                        ),
                )
                .child(
                    div()
                        .mt_5()
                        .text_size(px(18.))
                        .line_height(px(28.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title.into()),
                )
                .child(
                    div()
                        .mt_2()
                        .text_sm()
                        .line_height(px(20.))
                        .text_color(colors.muted_foreground)
                        .child(hint()),
                )
                .child(
                    div()
                        .id(id)
                        .mt_6()
                        .h(px(36.))
                        .px_4()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .bg(colors.primary)
                        .text_color(colors.primary_foreground)
                        .shadow_xs()
                        .text_sm()
                        .font_weight(MEDIUM)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(if authenticating {
                            "Authenticating…"
                        } else {
                            action
                        })
                        .when(authenticating, |view| view.opacity(0.5))
                        .when(!authenticating, |view| {
                            view.cursor_pointer()
                                .hover(|style| style.bg(colors.primary.opacity(0.9)))
                                .on_click(on_unlock)
                        }),
                ),
        )
}

/// Whether the main window was taken off screen (closed to the background or hidden), rather
/// than covered, minimized or hidden with the rest of the app, which shipping does not lock on.
mod presence {
    use gpui::Window;

    #[cfg(target_os = "macos")]
    pub fn ordered_out(window: &Window) -> bool {
        use objc2::{
            class, msg_send,
            runtime::{AnyObject, Bool},
        };
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return false;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return false;
        };
        let view = handle.ns_view.as_ptr().cast::<AnyObject>();
        // SAFETY: the view belongs to this open window; these are read-only AppKit getters.
        unsafe {
            let ns_window: *mut AnyObject = msg_send![view, window];
            if ns_window.is_null() {
                return false;
            }
            let visible: Bool = msg_send![ns_window, isVisible];
            let minimized: Bool = msg_send![ns_window, isMiniaturized];
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let app_hidden: Bool = msg_send![app, isHidden];
            !visible.as_bool() && !minimized.as_bool() && !app_hidden.as_bool()
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn ordered_out(window: &Window) -> bool {
        !window.is_visible() && !window.is_minimized()
    }
}

#[cfg(test)]
mod tests;
