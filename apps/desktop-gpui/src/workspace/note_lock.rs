//! Note lock (`lock/notes.ts`, `lock-note.tsx`, `LockedNoteGate`): locking and unlocking a note
//! needs device authentication, and a locked note shows "Note is Locked" until the device owner
//! authenticates again. The reveal lasts until the app locks.

use desktop_runtime::{Result, ServiceError, SessionId, SessionSummary};
use gpui::{App, Div, Hsla, Task, Window, div, prelude::*, px, svg};
use serde_json::json;

use super::{
    library::OpenNote,
    mutations::{statement, transaction},
};
use crate::{
    application::lock::{self, NoteAccess},
    ui::theme::theme,
};

/// Replaces the open note while it is locked, or while lock state is still unknown.
pub(super) fn gate(summary: Option<&SessionSummary>, window: &Window, cx: &App) -> Option<Div> {
    let summary = summary?;
    let colors = theme(window);
    match lock::note_access(&summary.id, cx) {
        NoteAccess::Open => None,
        NoteAccess::Unknown => Some(div().size_full()),
        NoteAccess::Locked => {
            let id = summary.id.clone();
            Some(div().size_full().child(lock::screen(
                "note-lock",
                if summary.title.is_empty() {
                    gpui::SharedString::from("Note is Locked")
                } else {
                    gpui::SharedString::from(summary.title.clone())
                },
                "View Note",
                lock::authenticating(cx),
                colors,
                move |_, _, cx| lock::reveal_note(id.clone(), cx).detach(),
            )))
        }
    }
}

/// Clicking a locked note authenticates before it opens (the timeline's `revealLockedNote`).
/// Other ways in show "Note is Locked" instead.
pub(super) fn reveal_first(id: &SessionId, cx: &mut App) -> Option<Task<bool>> {
    (lock::note_locked(id, cx) == Some(false)).then(|| lock::reveal_note(id.clone(), cx))
}

/// Note windows have no lock screen, so a locked note authenticates before one opens; showing
/// any locked note in Finder authenticates every time. `None` lets the request through.
pub(super) fn guard(event: &OpenNote, cx: &mut App) -> Option<Task<bool>> {
    match event {
        OpenNote::Window(id) => reveal_first(id, cx),
        OpenNote::Reveal(id)
            if lock::note_locked(id, cx).is_some() || !lock::note_lock_known(cx) =>
        {
            Some(lock::authenticate(lock::OPEN_APP, cx))
        }
        _ => None,
    }
}

/// The Lock / Unlock Note row: `(icon, label, disabled)`, offered only with device authentication.
pub(super) fn menu_row(id: &SessionId, cx: &App) -> Option<(&'static str, &'static str, bool)> {
    if !lock::available(cx) {
        return None;
    }
    let disabled = lock::authenticating(cx) || !lock::note_lock_known(cx);
    Some(if lock::note_locked(id, cx).is_some() {
        ("editor/LockKeyholeOpenIcon.svg", "Unlock Note", disabled)
    } else {
        ("editor/LockPasswordIcon.svg", "Lock Note", disabled)
    })
}

/// `setSessionLocked`: authenticates, then stores the flag.
pub(super) fn toggle(id: SessionId, cx: &mut App) -> Task<Result<bool>> {
    let locked = lock::note_locked(&id, cx).is_none();
    let Some(runtime) = lock::runtime(cx) else {
        return Task::ready(Err(ServiceError::Closed));
    };
    let reason = if locked {
        lock::LOCK_NOTE
    } else {
        lock::UNLOCK_NOTE
    };
    let authenticated = lock::authenticate(reason, cx);
    cx.spawn(async move |cx| {
        if !authenticated.await {
            return Ok(false);
        }
        let session = id.clone();
        runtime
            .submit(move |services| async move {
                transaction(
                    &services.executor,
                    vec![statement(
                        "UPDATE sessions SET locked = ? WHERE id = ? AND deleted_at IS NULL".into(),
                        vec![json!(i64::from(locked)), json!(session.0)],
                        Some(1),
                    )],
                )
                .await
            })?
            .receive()
            .await?;
        cx.update(|cx| lock::note_lock_changed(id, locked, cx));
        Ok(true)
    })
}

/// Toggles from a menu, where a failure has nowhere else to go.
pub(super) fn toggle_from_menu(id: SessionId, cx: &mut App) {
    let task = toggle(id, cx);
    cx.spawn(async move |_| {
        if let Err(error) = task.await {
            tracing::warn!(%error, "the note lock could not be changed");
        }
    })
    .detach();
}

/// The timeline item's lock glyph: closed while locked, open once revealed.
pub(super) fn glyph(id: &SessionId, color: Hsla, cx: &App) -> Option<gpui::Svg> {
    let revealed = lock::note_locked(id, cx)?;
    Some(
        svg()
            .path(if revealed {
                "editor/LockKeyholeOpenIcon.svg"
            } else {
                "editor/LockPasswordIcon.svg"
            })
            .size(px(14.))
            .flex_shrink_0()
            .text_color(color),
    )
}
