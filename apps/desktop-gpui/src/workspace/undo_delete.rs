//! Shipping's delete with undo (`session/hooks/useDeleteSession.ts`,
//! `store/zustand/undo-delete.ts`, `sidebar/toast/undo-delete-toast.tsx`): a note is
//! tombstoned at once, an Undo toast restores it for five seconds, then its folder is removed.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anlg_db_app::IgnoredCalendarItemKind;
use desktop_runtime::{Reply, Result, RuntimeHandle, SessionId};
use gpui::{Context, Task};
use serde_json::json;

use super::{
    mutations::failure,
    navigation::Route,
    shell::{WorkspaceAction, WorkspaceView},
};
use crate::{
    application::toasts,
    contracts::{MeetingIntent, WorkspaceEvent},
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
    /// Shipping's single delete: no confirmation, an Undo toast instead.
    pub(super) fn delete_notes(&mut self, ids: &[SessionId], batch: bool, cx: &mut Context<Self>) {
        let ids = ids
            .iter()
            .filter(|id| {
                !self
                    .deletions
                    .iter()
                    .any(|pending| pending.notes.iter().any(|(pending, _)| pending == *id))
            })
            .cloned()
            .collect::<Vec<_>>();
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
