//! Shipping's sync conflict banner and version history dialog
//! (`session/components/{note-conflict-banner,version-history-dialog}.tsx`, with the data
//! from `session/queries/conflicts.ts`).

use std::sync::Arc;

use desktop_runtime::{
    CancellationToken, RenameSession, Reply, Result, RuntimeHandle, ServiceError, SessionId,
};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{
    AnyElement, AnyView, Context, Entity, FocusHandle, Hsla, MouseButton, ScrollHandle,
    SharedString, Subscription, Window, anchored, deferred, div, point, prelude::*, px, rgb,
};
use serde_json::{Value, json};

use super::{NoteEvent, NoteView, icon, is_dark};
use crate::{
    chat::state::distance_to_now,
    editor::EditorPane,
    ui::theme::{MEDIUM, Theme},
};

/// Note documents use id == session_id, so both conflict kinds key off the session id.
const CONFLICTS_SQL: &str = "SELECT c.id, c.field_name, c.edited_at_ms, c.value_json,
    c.created_at, COALESCE(note.body_format, 'prosemirror_json') AS body_format
  FROM e2ee_field_conflicts c
  LEFT JOIN session_documents note
    ON note.id = c.row_id AND note.kind = 'note' AND note.deleted_at IS NULL
  WHERE c.row_id = ? AND c.resolved_at IS NULL
    AND ((c.table_name = 'session_documents' AND c.field_name = 'body')
      OR (c.table_name = 'sessions' AND c.field_name = 'title'))
  ORDER BY c.created_at DESC, c.id DESC";

const VERSIONS_SQL: &str = "SELECT id, body, body_format, source, created_at
  FROM session_document_versions WHERE document_id = ?
  ORDER BY created_at DESC, id DESC LIMIT 50";

const PREVIEW_LENGTH: usize = 160;

#[derive(Clone, Debug, PartialEq)]
struct Conflict {
    id: Arc<str>,
    title: bool,
    at_ms: i64,
    value: Arc<str>,
    body_format: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    id: Arc<str>,
    label: &'static str,
    at_ms: i64,
    body: Arc<str>,
    body_format: Arc<str>,
}

/// The open note's unresolved sync conflicts and its version history dialog.
#[derive(Default)]
pub(super) struct Versions {
    conflicts: Vec<Conflict>,
    cancel: CancellationToken,
    /// The memo's save conflict, mirrored so the banner re-renders only when it changes.
    memo_conflict: bool,
    memo_observer: Option<Subscription>,
    busy: bool,
    history: Option<History>,
}

struct History {
    rows: Vec<Value>,
    entries: Vec<(Entry, SharedString)>,
    restoring: bool,
    error: Option<String>,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    scroll: ScrollHandle,
    cancel: CancellationToken,
}

impl Drop for Versions {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(history) = &self.history {
            history.cancel.cancel();
        }
    }
}

impl NoteView {
    /// Follows the shown note's unresolved conflicts, replacing any earlier note's.
    pub(super) fn watch_conflicts(&mut self, id: SessionId, cx: &mut Context<Self>) {
        self.versions = Versions::default();
        let cancel = self.versions.cancel.clone();
        watch_rows(
            &self.runtime,
            CONFLICTS_SQL,
            vec![json!(id)],
            cancel,
            cx,
            |this, rows, cx| {
                if let Ok(rows) = rows {
                    this.versions.conflicts = rows.iter().filter_map(conflict).collect();
                    this.refresh_history(cx);
                    cx.notify();
                }
            },
        );
    }

    /// The memo editor's save conflicts show in the same banner.
    pub(super) fn observe_memo(&mut self, content: &AnyView, cx: &mut Context<Self>) {
        let Ok(editor) = content.clone().downcast::<EditorPane>() else {
            return;
        };
        self.versions.memo_conflict = editor.read(cx).save_conflict();
        self.versions.memo_observer = Some(cx.observe(&editor, |this, editor, cx| {
            let conflict = editor.read(cx).save_conflict();
            if conflict != this.versions.memo_conflict {
                this.versions.memo_conflict = conflict;
                cx.notify();
            }
        }));
    }

    fn memo(&self) -> Option<Entity<EditorPane>> {
        self.content.clone()?.downcast::<EditorPane>().ok()
    }

    pub(super) fn open_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.current else {
            return;
        };
        let cancel = CancellationToken::new();
        let focus = cx.focus_handle();
        self.versions.history = Some(History {
            rows: Vec::new(),
            entries: Vec::new(),
            restoring: false,
            error: None,
            focus: focus.clone(),
            return_focus: window.focused(cx),
            scroll: ScrollHandle::new(),
            cancel: cancel.clone(),
        });
        focus.focus(window, cx);
        watch_rows(
            &self.runtime,
            VERSIONS_SQL,
            vec![json!(session.summary.id)],
            cancel,
            cx,
            |this, rows, cx| {
                if let (Some(history), Ok(rows)) = (&mut this.versions.history, rows) {
                    history.rows = rows;
                    this.refresh_history(cx);
                }
            },
        );
        cx.notify();
    }

    /// Rebuilds the dialog's entries when versions or conflicts change.
    fn refresh_history(&mut self, cx: &mut Context<Self>) {
        let conflicts = &self.versions.conflicts;
        if let Some(history) = &mut self.versions.history {
            history.entries = entries(conflicts, &history.rows)
                .into_iter()
                .map(|entry| {
                    let preview = preview(&entry.body, &entry.body_format);
                    (entry, preview.into())
                })
                .collect();
            cx.notify();
        }
    }

    fn close_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(history) = self.versions.history.take() {
            history.cancel.cancel();
            if let Some(focus) = history.return_focus {
                focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// "Keep this version": the shown note stays; its open conflicts are dismissed.
    fn keep_this_version(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &self.current else {
            return;
        };
        if let Some(memo) = self.memo().filter(|memo| memo.read(cx).save_conflict()) {
            memo.update(cx, |memo, cx| memo.keep_draft(cx));
        }
        if self.versions.conflicts.is_empty() {
            return;
        }
        let reply = resolve_all(&self.runtime, session.summary.id.clone());
        self.run_conflict_action(async move { reply?.receive().await }, cx);
    }

    /// "Use the other version": the newest conflicting value replaces the shown one.
    fn use_other_version(&mut self, cx: &mut Context<Self>) {
        if let Some(memo) = self.memo().filter(|memo| memo.read(cx).save_conflict()) {
            memo.update(cx, |memo, cx| memo.use_external(cx));
            return;
        }
        let (Some(session), Some(conflict)) = (&self.current, self.versions.conflicts.first())
        else {
            return;
        };
        let runtime = self.runtime.clone();
        let id = conflict.id.clone();
        if conflict.title {
            if self.has_unsaved_title(cx) {
                self.message = "Save or restore your title before using the other version.".into();
                cx.notify();
                return;
            }
            let rename = runtime.rename_session(RenameSession {
                base: session.summary.clone(),
                title: conflict.value.clone(),
            });
            self.versions.busy = true;
            cx.notify();
            cx.spawn(async move |this, cx| {
                let renamed = async { rename?.receive().await }.await;
                let resolved = match &renamed {
                    Ok(_) => async { resolve_one(&runtime, id)?.receive().await }.await,
                    Err(_) => Ok(()),
                };
                let _ = this.update(cx, |this, cx| {
                    this.versions.busy = false;
                    // A saved title is shown even if dismissing the conflict failed.
                    let error = match renamed {
                        Ok(session) => {
                            let session = Arc::new(session);
                            this.title.update(cx, |title, cx| {
                                title.set_text(session.summary.title.to_string(), cx)
                            });
                            this.current = Some(session.clone());
                            cx.emit(NoteEvent::TitleUpdated(session));
                            resolved.err()
                        }
                        Err(error) => Some(error),
                    };
                    if let Some(error) = error {
                        this.message = format!("Could not resolve the conflict: {error}");
                    }
                    cx.notify();
                });
            })
            .detach();
        } else {
            let Some(memo) = self.memo() else {
                return;
            };
            let restore = memo.update(cx, |memo, cx| {
                memo.restore(conflict.value.clone(), conflict.body_format.clone(), cx)
            });
            self.run_conflict_action(
                async move {
                    restore
                        .await
                        .map_err(|error| ServiceError::Failed(error.into()))?;
                    resolve_one(&runtime, id)?.receive().await
                },
                cx,
            );
        }
    }

    fn run_conflict_action(
        &mut self,
        action: impl Future<Output = Result<()>> + 'static,
        cx: &mut Context<Self>,
    ) {
        self.versions.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = action.await;
            let _ = this.update(cx, |this, cx| {
                this.versions.busy = false;
                if let Err(error) = result {
                    this.message = format!("Could not resolve the conflict: {error}");
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn restore_entry(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(memo), Some(history)) = (self.memo(), &mut self.versions.history) else {
            return;
        };
        let Some((entry, _)) = history.entries.get(index).cloned() else {
            return;
        };
        history.restoring = true;
        history.error = None;
        let restore = memo.update(cx, |memo, cx| {
            memo.restore(entry.body, entry.body_format, cx)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = restore.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(()) => this.close_history(window, cx),
                    Err(error) => {
                        if let Some(history) = &mut this.versions.history {
                            history.restoring = false;
                            history.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Shipping's `NoteConflictBanner`, above the note's content.
    pub(super) fn conflict_banner(
        &self,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let synced = !self.versions.conflicts.is_empty();
        if !synced && !self.versions.memo_conflict {
            return None;
        }
        let dark = is_dark(colors);
        let amber: Hsla = rgb(0xf59e0b).into();
        let busy = self.versions.busy;
        // A memo save that lost to another revision has no shipping counterpart; nothing was
        // kept yet, so it says so.
        let text = if synced {
            "This note was edited on another device at the same time. The later edit was kept."
        } else {
            "This note was changed somewhere else while you were editing. Your edits are not saved yet."
        };
        Some(
            div()
                .flex_shrink_0()
                .px(px(12.))
                .pt_1()
                .pb_2()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .border_1()
                        .border_color(amber.opacity(0.35))
                        .bg(amber.opacity(0.1))
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .items_start()
                                .gap(px(10.))
                                .child(
                                    icon(
                                        "sharing/TriangleAlertIcon.svg",
                                        16.,
                                        rgb(if dark { 0xfbbf24 } else { 0xd97706 }).into(),
                                    )
                                    .mt(px(2.)),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_size(px(13.))
                                        .line_height(px(20.))
                                        .font_weight(MEDIUM)
                                        .text_color(colors.foreground)
                                        .child(text),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .justify_end()
                                .gap_2()
                                .child(
                                    button(
                                        "conflict-keep",
                                        "Keep this version",
                                        true,
                                        busy,
                                        colors,
                                    )
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.keep_this_version(cx)),
                                    ),
                                )
                                .child(
                                    button(
                                        "conflict-use-other",
                                        "Use the other version",
                                        true,
                                        busy,
                                        colors,
                                    )
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.use_other_version(cx)),
                                    ),
                                )
                                .child(
                                    button("conflict-history", "History", false, false, colors)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_history(window, cx)
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Shipping's `VersionHistoryDialog`, over the whole window.
    pub(super) fn history_dialog(
        &self,
        colors: Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let history = self.versions.history.as_ref()?;
        let size = window.viewport_size();
        let now = chrono::Utc::now();
        let list =
            if history.entries.is_empty() {
                div()
                    .py_6()
                    .text_center()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(colors.muted_foreground)
                    .child("No earlier versions yet.")
                    .into_any_element()
            } else {
                div()
                    .id("version-history-list")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&history.scroll)
                    .children(history.entries.iter().enumerate().map(
                        |(index, (entry, preview))| {
                            let at = chrono::DateTime::from_timestamp_millis(entry.at_ms)
                                .filter(|_| entry.at_ms > 0)
                                .map(|at| distance_to_now(at, now));
                            div()
                                .flex()
                                .flex_shrink_0()
                                .items_start()
                                .justify_between()
                                .gap_3()
                                .py_3()
                                .when(index > 0, |row| {
                                    row.border_t_1().border_color(colors.border.opacity(0.6))
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_xs()
                                                .line_height(px(16.))
                                                .font_weight(MEDIUM)
                                                .child(entry.label),
                                        )
                                        .child(
                                            div()
                                                .mt(px(2.))
                                                .text_size(px(11.))
                                                .line_height(px(16.5))
                                                .text_color(colors.muted_foreground)
                                                .children(at),
                                        )
                                        .child(
                                            div()
                                                .mt_1()
                                                .text_xs()
                                                .line_height(px(20.))
                                                .text_color(colors.muted_foreground)
                                                .line_clamp(3)
                                                .child(if preview.is_empty() {
                                                    SharedString::from("Empty note")
                                                } else {
                                                    preview.clone()
                                                }),
                                        ),
                                )
                                .child(
                                    button(
                                        ("version-restore", index),
                                        "Restore",
                                        true,
                                        history.restoring,
                                        colors,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.restore_entry(index, window, cx)
                                        },
                                    )),
                                )
                        },
                    ))
                    .into_any_element()
            };
        let dialog = div()
            .id("version-history")
            .track_focus(&history.focus)
            .relative()
            .w_full()
            .max_w(px(512.))
            .max_h(px(560.).min(size.height - px(80.)))
            .flex()
            .flex_col()
            .gap_4()
            .overflow_hidden()
            .p_6()
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .shadow_lg()
            .text_color(colors.foreground)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.close_history(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .text_center()
                    .child(
                        // `text-base` overrides the title's `leading-none` in shipping's merge.
                        div()
                            .text_base()
                            .line_height(px(24.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Version history"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child("Earlier versions of this note, newest first."),
                    ),
            )
            .child(list)
            .children(history.error.clone().map(|error| {
                div()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(rgb(0xef4444))
                    .child(error)
            }))
            .child(
                div()
                    .id("version-history-close")
                    .absolute()
                    .top_4()
                    .right_4()
                    .opacity(0.7)
                    .hover(|style| style.opacity(1.))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| this.close_history(window, cx)))
                    .child(icon("editor/XIcon.svg", 16., colors.foreground)),
            );
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("version-history-backdrop")
                        .w(size.width)
                        .h(size.height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::black().opacity(0.8))
                        .occlude()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| this.close_history(window, cx)),
                        )
                        .child(dialog),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

/// Shipping's small `outline` and `ghost` buttons.
fn button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    outline: bool,
    disabled: bool,
    colors: Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(28.))
        .px_2()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .text_xs()
        .line_height(px(16.))
        .font_weight(MEDIUM)
        .text_color(colors.foreground)
        .when(outline, |button| {
            button
                .border_1()
                .border_color(colors.input)
                .bg(colors.background)
                .shadow_xs()
        })
        .when_else(
            disabled,
            |button| button.opacity(0.5).cursor_not_allowed(),
            |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.accent).text_color(colors.accent_foreground))
            },
        )
        .child(label)
}

fn conflict(row: &Value) -> Option<Conflict> {
    let text = |key: &str| Arc::<str>::from(row[key].as_str().unwrap_or_default());
    let raw = row["value_json"].as_str().unwrap_or("null");
    let value = match serde_json::from_str::<Value>(raw) {
        Ok(Value::String(value)) => value.into(),
        _ => raw.into(),
    };
    Some(Conflict {
        id: row["id"].as_str()?.into(),
        title: row["field_name"] == "title",
        at_ms: row["edited_at_ms"]
            .as_i64()
            .unwrap_or_else(|| timestamp(row["created_at"].as_str().unwrap_or_default())),
        value,
        body_format: text("body_format"),
    })
}

/// The dialog lists the other devices' body versions with the stored ones, newest first.
fn entries(conflicts: &[Conflict], versions: &[Value]) -> Vec<Entry> {
    let text = |row: &Value, key: &str| Arc::<str>::from(row[key].as_str().unwrap_or_default());
    let mut entries: Vec<Entry> = conflicts
        .iter()
        .filter(|conflict| !conflict.title)
        .map(|conflict| Entry {
            id: conflict.id.clone(),
            label: "Other device's version",
            at_ms: conflict.at_ms,
            body: conflict.value.clone(),
            body_format: conflict.body_format.clone(),
        })
        .chain(versions.iter().map(|row| Entry {
            id: text(row, "id"),
            label: if row["source"] == "sync" {
                "Synced"
            } else {
                "This device"
            },
            at_ms: timestamp(row["created_at"].as_str().unwrap_or_default()),
            body: text(row, "body"),
            body_format: text(row, "body_format"),
        }))
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.at_ms));
    entries
}

fn timestamp(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value).map_or(0, |at| at.timestamp_millis())
}

/// Shipping's `previewFromBody`: the text, whitespace collapsed, cut at 160 characters.
fn preview(body: &str, format: &str) -> String {
    fn visit(node: &Value, parts: &mut String) {
        if let Some(text) = node["text"].as_str() {
            parts.push_str(text);
        }
        if let Some(children) = node["content"].as_array() {
            for child in children {
                visit(child, parts);
            }
            parts.push(' ');
        }
    }
    let text = match serde_json::from_str::<Value>(body) {
        Ok(doc) if format == "prosemirror_json" => {
            let mut parts = String::new();
            visit(&doc, &mut parts);
            parts
        }
        _ => body.to_owned(),
    };
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= PREVIEW_LENGTH {
        return collapsed;
    }
    let cut: String = collapsed.chars().take(PREVIEW_LENGTH).collect();
    format!("{}...", cut.trim_end())
}

fn resolve_all(runtime: &RuntimeHandle, session: SessionId) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let mut transaction = services
            .db
            .pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(failure)?;
        anlg_db_app::resolve_session_note_conflicts(&mut transaction, &session.0)
            .await
            .map_err(failure)?;
        transaction.commit().await.map_err(failure)
    })
}

fn resolve_one(runtime: &RuntimeHandle, conflict: Arc<str>) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let mut connection = services.db.pool().acquire().await.map_err(failure)?;
        anlg_db_app::resolve_session_conflict(&mut connection, &conflict)
            .await
            .map_err(failure)?;
        Ok(())
    })
}

fn failure(error: sqlx::Error) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

/// Follows a parameterized live query until `cancel`, handing each snapshot to `apply`.
fn watch_rows(
    runtime: &RuntimeHandle,
    sql: &'static str,
    params: Vec<Value>,
    cancel: CancellationToken,
    cx: &mut Context<NoteView>,
    apply: impl Fn(&mut NoteView, std::result::Result<Vec<Value>, ServiceError>, &mut Context<NoteView>)
    + 'static,
) {
    let reply = runtime.watch_query(sql.into(), params);
    cx.spawn(async move |this, cx| {
        let mut watch = match async { reply?.receive().await }.await {
            Ok(watch) => watch,
            Err(error) => {
                let _ = this.update(cx, |this, cx| apply(this, Err(error), cx));
                return;
            }
        };
        while !cancel.is_cancelled() {
            let rows = watch.snapshots.borrow_and_update().rows.clone();
            let error = watch
                .terminal_error()
                .or_else(|| watch.errors.try_recv().ok());
            let terminal = error.is_some();
            let result = error.map_or_else(|| Ok(rows.to_vec()), Err);
            if this.update(cx, |this, cx| apply(this, result, cx)).is_err() || terminal {
                break;
            }
            match select(
                watch.snapshots.changed().boxed(),
                cancel.cancelled().boxed(),
            )
            .await
            {
                Either::Left((Ok(()), _)) => {}
                _ => break,
            }
        }
        let _ = watch.unsubscribe().await;
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use desktop_runtime::Profile;
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn history_lists_unresolved_body_conflicts_with_stored_versions_newest_first() {
        block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let (runtime, ready) = RuntimeHandle::start(Profile {
                database: directory.path().join("versions.sqlite"),
            })
            .unwrap();
            ready.receive().await.unwrap();
            let read = |sql: &'static str| {
                runtime
                    .submit(move |services| async move {
                        services
                            .executor
                            .execute(sql.into(), vec![json!("s")])
                            .await
                            .map_err(|error| ServiceError::Failed(error.to_string().into()))
                    })
                    .unwrap()
                    .receive()
            };
            runtime
                .submit(|services| async move {
                    sqlx::raw_sql(
                        "INSERT INTO sessions (id, title) VALUES ('s', 'Note'), ('t', 'Other');
                         INSERT INTO session_documents (id, session_id, kind, body_format, body)
                           VALUES ('s', 's', 'note', 'prosemirror_json', '{\"type\":\"doc\"}');
                         INSERT INTO e2ee_field_conflicts
                           (id, table_name, row_id, field_name, edited_at_ms, value_json, created_at, resolved_at)
                         VALUES
                           ('body', 'session_documents', 's', 'body', 1700000300000,
                             '\"{\\\"type\\\":\\\"doc\\\",\\\"content\\\":[{\\\"type\\\":\\\"paragraph\\\",\\\"content\\\":[{\\\"type\\\":\\\"text\\\",\\\"text\\\":\\\"Theirs\\\"}]}]}\"',
                             '2023-11-14T22:15:00.000Z', NULL),
                           ('title', 'sessions', 's', 'title', NULL, '\"Their title\"',
                             '2023-11-14T22:20:00.000Z', NULL),
                           ('done', 'session_documents', 's', 'body', NULL, '\"old\"',
                             '2023-11-14T22:25:00.000Z', '2023-11-14T22:26:00.000Z'),
                           ('elsewhere', 'session_documents', 't', 'body', NULL, '\"other\"',
                             '2023-11-14T22:25:00.000Z', NULL);
                         INSERT INTO session_document_versions
                           (id, document_id, session_id, body_format, body, source, created_at)
                         VALUES
                           ('older', 's', 's', 'markdown', 'First   draft', 'local', '2023-11-14T22:10:00.000Z'),
                           ('newer', 's', 's', 'prosemirror_json', '{}', 'sync', '2023-11-14T22:30:00.000Z');",
                    )
                    .execute(services.db.pool())
                    .await
                    .map_err(failure)?;
                    Ok(())
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            let conflicts: Vec<_> = read(CONFLICTS_SQL)
                .await
                .unwrap()
                .iter()
                .filter_map(conflict)
                .collect();
            assert_eq!(
                conflicts
                    .iter()
                    .map(|c| (c.id.as_ref(), c.title))
                    .collect::<Vec<_>>(),
                [("title", true), ("body", false)]
            );
            let entries = entries(&conflicts, &read(VERSIONS_SQL).await.unwrap());
            assert_eq!(
                entries
                    .iter()
                    .map(|entry| (entry.id.as_ref(), entry.label))
                    .collect::<Vec<_>>(),
                [
                    ("newer", "Synced"),
                    ("body", "Other device's version"),
                    ("older", "This device"),
                ]
            );
            assert_eq!(preview(&entries[1].body, &entries[1].body_format), "Theirs");
            assert_eq!(
                preview(&entries[2].body, &entries[2].body_format),
                "First draft"
            );
            runtime.shutdown().await.unwrap();
        });
    }
}
