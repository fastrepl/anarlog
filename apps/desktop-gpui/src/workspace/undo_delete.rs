//! Shipping's delete with undo (`session/hooks/useDeleteSession.ts`,
//! `store/zustand/undo-delete.ts`, `sidebar/toast/undo-delete-toast.tsx`): a note is
//! tombstoned at once, an Undo toast restores it for five seconds, then its folder is removed.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anlg_db_app::IgnoredCalendarItemKind;
use desktop_runtime::{Reply, Result, RuntimeHandle, SessionId};
use gpui::{App, Context, Entity, Task};
use serde_json::json;

use super::{
    mutations::failure,
    navigation::Route,
    open_note::NoteView,
    shell::{WorkspaceAction, WorkspaceView},
};
use crate::{
    application::toasts,
    contracts::{MeetingIntent, WorkspaceEvent},
    editor::{holds::NoteHolds, persistence::Flush},
    note_window::NoteWindow,
    product::sharing::SharingCloud,
};

/// `UNDO_TIMEOUT_MS`.
pub const UNDO_TIMEOUT: Duration = Duration::from_secs(5);

const CHILD_TABLES: &[&str] = &[
    "session_documents",
    "transcripts",
    "session_participants",
    "session_tags",
    "action_items",
    "session_attachments",
];

/// Tombstones the note and its records with one timestamp so undo restores exactly them, and
/// hides its calendar event like shipping. `None` when the note was already deleted.
pub fn soft_delete(
    runtime: &RuntimeHandle,
    id: SessionId,
    tombstone: Arc<str>,
) -> Result<Reply<Option<Arc<str>>>> {
    runtime.submit(move |services| async move {
        let pool = services.db.pool();
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.map_err(failure)?;
        let Some(row) = anlg_db_app::load_session_delete_row(&mut tx, &id.0)
            .await
            .map_err(failure)?
        else {
            tx.rollback().await.map_err(failure)?;
            return Ok(None);
        };
        let tracking: Option<String> = sqlx::query_scalar(
            "SELECT CASE WHEN json_valid(event_json) THEN json_extract(event_json, '$.tracking_id') END FROM sessions WHERE id = ?",
        )
        .bind(id.0.as_ref())
        .fetch_one(&mut *tx)
        .await
        .map_err(failure)?;
        let deleted = tombstone_rows(&mut tx, &id.0, &tombstone, false).await?;
        tx.commit().await.map_err(failure)?;
        if deleted != 1 {
            return Ok(None);
        }
        if let Some(tracking) = tracking.filter(|tracking| !tracking.is_empty()) {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            anlg_db_app::update_ignored_calendar_item(
                pool,
                IgnoredCalendarItemKind::Events,
                &tracking,
                true,
                &now,
            )
            .await
            .map_err(failure)?;
        }
        Ok(Some(row.title.into()))
    })
}

/// Clears only the rows this deletion tombstoned.
pub fn restore(runtime: &RuntimeHandle, id: SessionId, tombstone: Arc<str>) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let mut tx = services
            .db
            .pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(failure)?;
        let restored = tombstone_rows(&mut tx, &id.0, &tombstone, true).await?;
        let alive = anlg_db_app::session_is_alive(&mut tx, &id.0)
            .await
            .map_err(failure)?;
        tx.commit().await.map_err(failure)?;
        if restored == 1 || alive {
            Ok(())
        } else {
            Err(failure("The note was never deleted"))
        }
    })
}

async fn tombstone_rows(
    tx: &mut sqlx::SqliteConnection,
    id: &str,
    tombstone: &str,
    restore: bool,
) -> Result<u64> {
    for table in CHILD_TABLES {
        anlg_db_app::update_session_child_tombstone(tx, table, id, tombstone, restore)
            .await
            .map_err(failure)?;
    }
    anlg_db_app::update_entity_mentions_tombstone(tx, id, tombstone, restore)
        .await
        .map_err(failure)?;
    anlg_db_app::update_sessions_tombstone(tx, id, tombstone, restore)
        .await
        .map_err(failure)
}

/// `finalizeSessionDeletion`: once undo has expired, removes the deleted note's folder.
pub fn finalize(runtime: &RuntimeHandle, id: SessionId) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let rows = services
            .executor
            .execute(
                "SELECT id FROM sessions WHERE id = ? AND deleted_at IS NOT NULL".into(),
                vec![json!(id.0)],
            )
            .await
            .map_err(failure)?;
        if rows.is_empty() {
            return Ok(());
        }
        let fs = super::folders::filesystem(&services.executor).await?;
        tokio::task::spawn_blocking(move || {
            let path = fs.resolve_session_dir(&id.0).map_err(failure)?;
            anlg_fs_sync_core::session::delete_session_dir(&path).map_err(failure)
        })
        .await
        .map_err(failure)?
    })
}

/// Notes deleted together, restored together by one Undo.
pub(super) struct Pending {
    key: Arc<str>,
    notes: Vec<(SessionId, Arc<str>)>,
    title: Arc<str>,
    batch: bool,
    added: Instant,
    _timeout: Task<()>,
}

impl Pending {
    fn toast_id(&self) -> String {
        format!("undo-delete:{}", self.key)
    }

    fn label(&self) -> String {
        if self.batch {
            let count = self.notes.len();
            format!(
                "{count} {} deleted",
                if count == 1 { "note" } else { "notes" }
            )
        } else if self.title.trim().is_empty() {
            "Untitled deleted".into()
        } else {
            format!("{} deleted", self.title)
        }
    }
}

impl WorkspaceView {
    /// Shipping's single delete: no confirmation, an Undo toast instead. Edits still waiting
    /// on the save debounce are saved first, with every note in the deletion read-only
    /// meanwhile: the tombstone rejects later saves, and Undo would restore the older body.
    pub(super) fn delete_notes(&mut self, ids: &[SessionId], batch: bool, cx: &mut Context<Self>) {
        let ids = self.not_deleting(ids);
        if ids.is_empty() {
            return;
        }
        let flushes = self.flush_drafts(&ids, cx);
        if flushes.is_empty() {
            self.tombstone(&ids, batch, cx);
            return;
        }
        let hold = NoteHolds::hold(ids.clone(), cx);
        cx.spawn(async move |this, cx| {
            let mut saved = true;
            for flush in flushes {
                saved &= matches!(flush.await, Ok(Ok(())));
            }
            let _ = this.update(cx, |this, cx| {
                if saved {
                    this.tombstone(&ids, batch, cx);
                } else {
                    toasts::error(
                        "Could not save this note's latest edits, so it was not deleted.",
                        cx,
                    );
                }
            });
            // After the tombstone, so an edit typed from here on cannot land before it.
            cx.update(|cx| hold.release(cx));
        })
        .detach();
    }

    fn not_deleting(&self, ids: &[SessionId]) -> Vec<SessionId> {
        ids.iter()
            .filter(|id| {
                !self
                    .deletions
                    .iter()
                    .any(|pending| pending.notes.iter().any(|(pending, _)| pending == *id))
            })
            .cloned()
            .collect()
    }

    /// Starts saving the notes' drafts in this window and in their note windows.
    fn flush_drafts(&mut self, ids: &[SessionId], cx: &mut Context<Self>) -> Vec<Flush> {
        let mut flushes = Vec::new();
        for note in self.notes(cx) {
            flushes.extend(note.update(cx, |note, cx| note.flush_drafts(ids, cx)));
        }
        flushes
    }

    /// The note in this window and those in note windows.
    fn notes(&self, cx: &App) -> Vec<Entity<NoteView>> {
        let windows = cx.windows().into_iter().filter_map(|window| {
            window
                .downcast::<NoteWindow>()
                .and_then(|window| window.read(cx).ok().map(|view| view.note()))
        });
        std::iter::once(self.note.clone()).chain(windows).collect()
    }

    fn tombstone(&mut self, ids: &[SessionId], batch: bool, cx: &mut Context<Self>) {
        // A repeat delete may have started while the drafts saved.
        let ids = self.not_deleting(ids);
        if ids.is_empty() {
            return;
        }
        if ids.iter().any(|id| self.recording.contains(id)) {
            cx.emit(WorkspaceEvent::Meeting(MeetingIntent::Stop));
        }
        let open = ids
            .iter()
            .filter(|id| {
                self.navigation
                    .tabs
                    .iter()
                    .any(|tab| tab.route == Route::Session((*id).clone()))
            })
            .cloned()
            .collect::<Vec<_>>();
        let title = ids
            .first()
            .and_then(|id| self.note_title(id))
            .unwrap_or_default();
        let tombstone: Arc<str> = chrono::Utc::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            .into();
        let replies = ids
            .iter()
            .map(|id| soft_delete(&self.runtime, id.clone(), tombstone.clone()))
            .collect::<Vec<_>>();
        let notes = ids
            .iter()
            .map(|id| (id.clone(), tombstone.clone()))
            .collect();
        let key: Arc<str> = if batch {
            uuid::Uuid::new_v4().to_string().into()
        } else {
            ids[0].0.clone()
        };
        self.forget_sessions(&ids.clone().into(), cx);
        self.add_deletion(key.clone(), notes, title, batch, cx);
        cx.spawn(async move |this, cx| {
            let mut failed = Vec::new();
            let mut skipped = Vec::new();
            for (id, reply) in ids.into_iter().zip(replies) {
                match async { reply?.receive().await }.await {
                    Ok(Some(_)) => {}
                    // Already deleted elsewhere: nothing to undo.
                    Ok(None) => skipped.push(id),
                    Err(_) => failed.push(id),
                }
            }
            if failed.is_empty() && skipped.is_empty() {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                this.drop_from_deletion(&key, failed.iter().chain(&skipped), cx);
                if !failed.is_empty() {
                    toasts::error("Could not delete this note. Please try again.", cx);
                    if let Some(id) = failed.iter().find(|id| open.contains(id)) {
                        this.open_session(id.clone(), cx);
                    }
                    this.reload(cx);
                }
            });
        })
        .detach();
    }

    fn note_title(&self, id: &SessionId) -> Option<Arc<str>> {
        self.page
            .as_ref()
            .and_then(|page| page.items.iter().find(|item| &item.id == id))
            .map(|item| item.title.clone())
            .or_else(|| self.titles.get(id).cloned())
    }

    /// Drops deleted notes from tabs, history and caches, like `invalidateResource`.
    pub(super) fn forget_sessions(&mut self, ids: &Arc<[SessionId]>, cx: &mut Context<Self>) {
        let previous = self.current_route();
        self.navigation.remove_sessions(ids);
        self.note
            .update(cx, |note, cx| note.remove_sessions(ids, cx));
        for id in ids.iter() {
            self.titles.remove(id);
            self.title_order.retain(|cached| cached != id);
            self.dirty.remove(id);
        }
        cx.emit(WorkspaceEvent::SessionsDeleted(ids.clone()));
        if self.current_route() != previous
            && let Route::Session(id) = self.current_route()
        {
            self.note.update(cx, |note, cx| note.open(id, cx));
        }
        if self.pin_persistence {
            cx.emit(WorkspaceAction::PinnedChanged(
                self.navigation
                    .tabs
                    .iter()
                    .filter(|tab| tab.pinned && tab.route.persistent_pin())
                    .map(|tab| tab.route.clone())
                    .collect(),
            ));
        }
        self.sync_route(cx);
        cx.notify();
    }

    fn add_deletion(
        &mut self,
        key: Arc<str>,
        notes: Vec<(SessionId, Arc<str>)>,
        title: Arc<str>,
        batch: bool,
        cx: &mut Context<Self>,
    ) {
        let timeout = {
            let key = key.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(UNDO_TIMEOUT).await;
                let _ = this.update(cx, |this, cx| this.confirm_deletion(&key, cx));
            })
        };
        let pending = Pending {
            key: key.clone(),
            notes,
            title,
            batch,
            added: Instant::now(),
            _timeout: timeout,
        };
        let view = cx.entity().downgrade();
        toasts::show(
            toasts::Toast::info(pending.label())
                .id(pending.toast_id())
                .sticky()
                .dismissible(false)
                .gauge(pending.added, UNDO_TIMEOUT)
                .action("Undo", move |_, cx| {
                    let _ = view.update(cx, |this, cx| this.undo_deletion(&key, cx));
                }),
            cx,
        );
        self.deletions.push(pending);
    }

    fn take_deletion(&mut self, key: &str, cx: &mut Context<Self>) -> Option<Pending> {
        let index = self
            .deletions
            .iter()
            .position(|pending| pending.key.as_ref() == key)?;
        let pending = self.deletions.remove(index);
        toasts::dismiss(&pending.toast_id(), cx);
        Some(pending)
    }

    fn drop_from_deletion<'a>(
        &mut self,
        key: &str,
        ids: impl Iterator<Item = &'a SessionId>,
        cx: &mut Context<Self>,
    ) {
        let ids = ids.collect::<Vec<_>>();
        if let Some(pending) = self
            .deletions
            .iter_mut()
            .find(|pending| pending.key.as_ref() == key)
        {
            pending.notes.retain(|(id, _)| !ids.contains(&id));
            if pending.notes.is_empty() {
                self.take_deletion(key, cx);
            }
        }
    }

    /// The Undo action: restores the group and reopens its first note.
    fn undo_deletion(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(pending) = self.take_deletion(key, cx) else {
            return;
        };
        let restores = pending
            .notes
            .iter()
            .map(|(id, tombstone)| restore(&self.runtime, id.clone(), tombstone.clone()))
            .collect::<Vec<_>>();
        cx.spawn(async move |this, cx| {
            let mut remaining = pending.notes.clone();
            for reply in restores {
                if async { reply?.receive().await }.await.is_err() {
                    break;
                }
                remaining.remove(0);
            }
            let _ = this.update(cx, |this, cx| {
                if remaining.len() < pending.notes.len() {
                    this.open_session(pending.notes[0].0.clone(), cx);
                }
                if !remaining.is_empty() {
                    toasts::error("Could not restore deleted note", cx);
                    this.add_deletion(
                        pending.key.clone(),
                        remaining,
                        pending.title.clone(),
                        pending.batch,
                        cx,
                    );
                }
                this.reload(cx);
            });
        })
        .detach();
    }

    fn confirm_deletion(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(pending) = self.take_deletion(key, cx) else {
            return;
        };
        let ids = pending
            .notes
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        let finalized = ids
            .iter()
            .map(|id| finalize(&self.runtime, id.clone()))
            .collect::<Vec<_>>();
        // After the folders, like shipping; a failure never undoes the local deletion.
        let revoked = cx
            .try_global::<SharingCloud>()
            .map(|cloud| cloud.0.revoke_deleted_shares(ids));
        cx.spawn(async move |_, cx| {
            for reply in finalized {
                // Like shipping, a leftover folder is only logged.
                if let Err(error) = async { reply?.receive().await }.await {
                    tracing::warn!(%error, "deleted note folder was not removed");
                }
            }
            if let Some(revoked) = revoked
                && let Err(error) = async { revoked?.receive().await }.await
            {
                tracing::warn!(%error, "deleted note's shared link was not removed");
                cx.update(|cx| {
                    toasts::show(
                        toasts::Toast::warning(
                            "Note deleted, but its shared link could not be removed.",
                        )
                        .id("shared-link-removal-failed")
                        .sticky(),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    /// Quitting ends every undo window so no deleted note keeps its folder.
    pub fn confirm_pending_deletions(&mut self, cx: &mut Context<Self>) {
        let keys = self
            .deletions
            .iter()
            .map(|pending| pending.key.clone())
            .collect::<Vec<_>>();
        for key in keys {
            self.confirm_deletion(&key, cx);
        }
    }

    pub(super) fn request_delete_selected(
        &mut self,
        ids: Arc<[SessionId]>,
        cx: &mut Context<Self>,
    ) {
        if !ids.is_empty() {
            self.confirm_delete = Some(ids);
            cx.notify();
        }
    }

    pub(super) fn close_delete_confirmation(&mut self, cx: &mut Context<Self>) {
        if self.confirm_delete.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn confirm_delete_selected(&mut self, cx: &mut Context<Self>) {
        let Some(ids) = self.confirm_delete.take() else {
            return;
        };
        self.delete_notes(&ids, ids.len() > 1, cx);
        self.library
            .update(cx, |library, cx| library.clear_selection(cx));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use desktop_runtime::{CancellationToken, OpenSession, Profile};
    use futures::executor::block_on;
    use gpui::{AppContext as _, VisualTestContext};
    use serde_json::json;

    use super::*;
    use crate::{
        contracts::{EditorInit, LaneContext},
        editor::EditorPane,
    };

    // Saves and the tombstone run on the runtime's threads; allow for a loaded CI machine.
    fn settle(cx: &mut VisualTestContext, done: impl Fn(&mut VisualTestContext) -> bool) {
        for _ in 0..2000 {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("timed out");
    }

    /// Shuts the runtime down while GPUI's foreground tasks keep running. Blocking this thread on
    /// the drain can deadlock when it waits on work those tasks finish, so a hang fails instead.
    fn shut_down(runtime: &RuntimeHandle, cx: &mut VisualTestContext) {
        let (done, result) = std::sync::mpsc::channel();
        let runtime = runtime.clone();
        std::thread::spawn(move || {
            let _ = done.send(block_on(runtime.shutdown()));
        });
        for _ in 0..2000 {
            cx.run_until_parked();
            if let Ok(result) = result.try_recv() {
                result.unwrap();
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("shutdown timed out");
    }

    fn deleted(runtime: &RuntimeHandle, id: &SessionId) -> bool {
        let id = id.clone();
        let reply = runtime
            .submit(move |services| async move {
                let rows = services
                    .executor
                    .execute(
                        "SELECT deleted_at FROM sessions WHERE id = ?".into(),
                        vec![json!(id.0)],
                    )
                    .await
                    .map_err(failure)?;
                Ok(!rows[0]["deleted_at"].is_null())
            })
            .unwrap();
        block_on(reply.receive()).unwrap()
    }

    /// A ready runtime and the main window's workspace.
    fn workspace(
        cx: &mut gpui::TestAppContext,
    ) -> (
        tempfile::TempDir,
        RuntimeHandle,
        Entity<WorkspaceView>,
        &mut VisualTestContext,
    ) {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        let cx = cx.add_empty_window();
        let context = LaneContext {
            runtime: runtime.clone(),
        };
        let workspace =
            cx.update(|window, cx| cx.new(|cx| WorkspaceView::new(context, ready, window, cx)));
        settle(cx, |_| {
            matches!(
                *runtime.state().borrow(),
                desktop_runtime::RuntimeState::Ready
            )
        });
        (directory, runtime, workspace, cx)
    }

    /// The note's memo editor, once its document has loaded.
    fn memo(
        runtime: &RuntimeHandle,
        open: &OpenSession,
        cx: &mut VisualTestContext,
    ) -> Entity<EditorPane> {
        let editor = cx.update(|window, cx| {
            let init = EditorInit {
                session_id: open.summary.id.clone(),
                document: open.note.clone().unwrap(),
                return_focus: cx.focus_handle(),
            };
            let context = LaneContext {
                runtime: runtime.clone(),
            };
            cx.new(|cx| EditorPane::new(context, init, window, cx))
        });
        settle(cx, |cx| {
            editor.read_with(cx, |editor, _| editor.is_pristine())
        });
        editor
    }

    /// Types into the editor and reports whether it took the edit.
    fn type_into(editor: &Entity<EditorPane>, cx: &mut VisualTestContext) -> bool {
        editor.update_in(cx, |editor, window, cx| {
            gpui::EntityInputHandler::replace_text_in_range(editor, None, " and more", window, cx)
        });
        editor.read_with(cx, |editor, _| editor.is_dirty())
    }

    /// Opens the note in the main window with an unsaved edit.
    fn edit_open_note(
        runtime: &RuntimeHandle,
        workspace: &Entity<WorkspaceView>,
        open: &OpenSession,
        cx: &mut VisualTestContext,
    ) -> Entity<EditorPane> {
        let session = open.summary.id.clone();
        workspace.update(cx, |workspace, cx| {
            workspace.open_session(session.clone(), cx)
        });
        settle(cx, |cx| {
            workspace.read_with(cx, |workspace, cx| {
                workspace.note.read(cx).session_id() == Some(&session)
            })
        });
        let editor = memo(runtime, open, cx);
        workspace.update(cx, |workspace, cx| {
            workspace.set_editor_content(&session, editor.clone().into(), cx)
        });
        let body = json!({"type": "doc", "content": [
            {"type": "paragraph", "content": [{"type": "text", "text": "Typed just now"}]}
        ]});
        editor.update(cx, |editor, cx| {
            editor.replace_content(&body.to_string(), cx)
        });
        editor
    }

    /// Deleting a note within the save debounce of an edit lost the edit: its save ran after
    /// the tombstone and was rejected, so Undo restored the older body.
    #[gpui::test]
    fn deleting_a_note_saves_its_pending_edit_first(cx: &mut gpui::TestAppContext) {
        let (_directory, runtime, workspace, cx) = workspace(cx);
        let open = block_on(runtime.create_note("Draft".into()).unwrap().receive()).unwrap();
        let session = open.summary.id.clone();
        let editor = edit_open_note(&runtime, &workspace, &open, cx);
        workspace.update(cx, |workspace, cx| {
            workspace.delete_notes(std::slice::from_ref(&session), false, cx)
        });
        // Typing while that edit saves once slipped in and then missed the tombstone.
        type_into(&editor, cx);
        settle(cx, |_| deleted(&runtime, &session));
        assert!(
            !editor.read_with(cx, |editor, _| editor.is_dirty()),
            "no edit may be left behind the tombstone"
        );
        workspace.update(cx, |workspace, cx| workspace.undo_deletion(&session.0, cx));
        settle(cx, |_| !deleted(&runtime, &session));
        let restored = block_on(
            runtime
                .open_session(session.clone(), CancellationToken::new())
                .unwrap()
                .receive(),
        )
        .unwrap();
        assert!(restored.note.unwrap().body.contains("Typed just now"));
        drop(editor);
        shut_down(&runtime, cx);
    }

    /// A batch deletion held only its notes with unsaved edits: a clean one took an edit while
    /// another's saved, and the tombstone then rejected that edit's save.
    #[gpui::test]
    fn a_batch_deletion_holds_its_clean_notes_too(cx: &mut gpui::TestAppContext) {
        let (_directory, runtime, workspace, cx) = workspace(cx);
        let edited = block_on(runtime.create_note("Draft".into()).unwrap().receive()).unwrap();
        let clean = block_on(runtime.create_note("Clean".into()).unwrap().receive()).unwrap();
        let editor = edit_open_note(&runtime, &workspace, &edited, cx);
        // The clean note is open elsewhere, as in a note window.
        let other = memo(&runtime, &clean, cx);
        let ids = [edited.summary.id.clone(), clean.summary.id.clone()];
        workspace.update(cx, |workspace, cx| workspace.delete_notes(&ids, true, cx));
        assert!(
            !type_into(&other, cx),
            "a note in the deletion takes no edit while the deletion saves"
        );
        settle(cx, |_| ids.iter().all(|id| deleted(&runtime, id)));
        drop((editor, other));
        shut_down(&runtime, cx);
    }

    /// An editor created during a deletion's save, such as a summary shown meanwhile, was not
    /// held, so its edits missed the deletion's flushes.
    #[gpui::test]
    fn an_editor_opened_while_its_note_is_held_takes_no_edits(cx: &mut gpui::TestAppContext) {
        let (_directory, runtime, _workspace, cx) = workspace(cx);
        let open = block_on(runtime.create_note("Draft".into()).unwrap().receive()).unwrap();
        let hold = cx.update(|_, cx| NoteHolds::hold(vec![open.summary.id.clone()], cx));
        let editor = memo(&runtime, &open, cx);
        assert!(!type_into(&editor, cx));
        cx.update(|_, cx| hold.release(cx));
        assert!(type_into(&editor, cx), "the note is editable once released");
        block_on(editor.update(cx, |editor, _| editor.flush()))
            .unwrap()
            .unwrap();
        shut_down(&runtime, cx);
    }

    /// When deletions overlapped, the first to finish thawed the other's notes while that one
    /// still saved their drafts.
    #[gpui::test]
    fn a_note_held_by_two_deletions_stays_held_until_both_end(cx: &mut gpui::TestAppContext) {
        let (_directory, runtime, _workspace, cx) = workspace(cx);
        let open = block_on(runtime.create_note("Draft".into()).unwrap().receive()).unwrap();
        let other = block_on(runtime.create_note("Other".into()).unwrap().receive()).unwrap();
        let editor = memo(&runtime, &open, cx);
        let id = open.summary.id.clone();
        let batch = cx.update(|_, cx| NoteHolds::hold(vec![other.summary.id, id.clone()], cx));
        let single = cx.update(|_, cx| NoteHolds::hold(vec![id], cx));
        cx.update(|_, cx| batch.release(cx));
        assert!(!type_into(&editor, cx));
        cx.update(|_, cx| single.release(cx));
        assert!(type_into(&editor, cx), "the note is editable once both end");
        block_on(editor.update(cx, |editor, _| editor.flush()))
            .unwrap()
            .unwrap();
        shut_down(&runtime, cx);
    }
}
