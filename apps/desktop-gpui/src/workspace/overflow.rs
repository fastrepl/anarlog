//! `apps/desktop/src/session/components/outer-header/overflow`: the `…` menu
//! of the note header and the Delete flow with its undo toast
//! (`useDeleteSession` + `undo-delete-toast.tsx`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, BoxShadow, ClickEvent, Context, MouseButton, SharedString, Window, div, hsla,
    point, prelude::*, px,
};

use super::Workspace;
use super::menu::{Align, Entry, MenuSpec, Select, Submenu, Trailing};
use crate::theme::alpha;

/// `UNDO_TIMEOUT_MS`
pub(super) const UNDO_TIMEOUT: Duration = Duration::from_millis(5000);

/// A soft-deleted note the user can still bring back.
pub(crate) struct PendingDeletion {
    pub session_id: String,
    pub title: String,
    pub tombstone: String,
    pub added_at: Instant,
}

impl Workspace {
    pub(crate) fn toggle_overflow_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overflow_open = !self.overflow_open;
        self.overflow_submenu = None;
        if self.overflow_open
            && let Some(session_id) = self.selected.clone()
        {
            self.prepare_meeting_info(session_id, window, cx);
        }
        cx.notify();
    }

    fn close_overflow_menu(&mut self, cx: &mut Context<Self>) {
        if self.overflow_open {
            self.overflow_open = false;
            self.overflow_submenu = None;
            cx.notify();
        }
    }

    fn hover_overflow_submenu(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        if self.overflow_submenu != index {
            self.overflow_submenu = index;
            cx.notify();
        }
    }

    fn show_in_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self
            .selected
            .as_deref()
            .map(|id| self.store.session_dir(id))
        {
            cx.open_url(&format!("file://{}", path.display()));
        }
    }

    /// The items the Tauri menu shows for a note without audio, transcript,
    /// or an active recording; lock is only offered where the OS app lock
    /// exists, which Linux lacks. Folder and Meeting info open submenus.
    fn overflow_spec(&self) -> MenuSpec {
        let plain =
            |icon: &'static str, label: &'static str, on_select: Option<Select>| Entry::Item {
                icon: Some(icon),
                dim_icon: false,
                label: label.into(),
                trailing: Trailing::None,
                destructive: false,
                on_select,
                submenu: None,
            };
        // `showUploadActions`: no audio file, no transcript, and nothing
        // written in the current view (recording is not ported, so a meeting
        // is never in progress here).
        let show_upload = match &self.note {
            super::Note::Ready { preview, tab } => {
                let audio_exists =
                    anlg_fs_sync_core::audio::exists(&self.store.session_dir(&preview.session.id))
                        .unwrap_or(false);
                let note_has_content = match tab {
                    super::NoteTab::Memo => crate::db::has_note_content(&preview.memo_body, "json"),
                    super::NoteTab::Enhanced(id) => preview
                        .enhanced
                        .iter()
                        .find(|document| document.id == *id)
                        .is_some_and(|document| !document.blocks.is_empty()),
                    super::NoteTab::Transcript => false,
                };
                !audio_exists && !preview.has_transcript && !note_has_content
            }
            _ => false,
        };
        let mut spec = MenuSpec {
            id: "overflow-menu",
            width: 224.0,
            open_sub: self.overflow_submenu,
            on_hover_sub: Self::hover_overflow_submenu,
            on_close: Self::close_overflow_menu,
            entries: vec![
                Entry::Item {
                    icon: Some("folder"),
                    dim_icon: false,
                    label: "Folder".into(),
                    trailing: Trailing::Submenu,
                    destructive: false,
                    on_select: None,
                    submenu: Some(Submenu::Entries(self.folder_submenu_entries())),
                },
                Entry::Item {
                    icon: Some("calendar-blank"),
                    dim_icon: false,
                    label: "Meeting info".into(),
                    trailing: Trailing::Submenu,
                    destructive: false,
                    on_select: None,
                    // `DropdownMenuSubContent className="w-72"` with `MetadataPanelContent`.
                    submenu: Some(Submenu::Panel {
                        width: 288.0,
                        render: |this, cx| this.render_meeting_info_panel(cx),
                    }),
                },
                Entry::Separator,
                plain(
                    "file-arrow-down",
                    "Export",
                    Some(Box::new(|this, _, cx| this.open_export_dialog(cx))),
                ),
                Entry::Separator,
                plain("microphone", "Start listening", None),
                plain("waveform", "Upload audio", None),
                plain("file-text", "Upload transcript", None),
                Entry::Separator,
                plain(
                    "app-window",
                    "Open in New Window",
                    Some(Box::new(|this, _, cx| {
                        if let Some(id) = this.selected.clone() {
                            this.open_note_window(id, cx);
                        }
                    })),
                ),
                plain(
                    "folder-open",
                    if cfg!(target_os = "macos") {
                        "Show in Finder"
                    } else {
                        "Show in folder"
                    },
                    Some(Box::new(|this, _, cx| this.show_in_folder(cx))),
                ),
                Entry::Item {
                    icon: Some("trash"),
                    dim_icon: false,
                    label: "Delete".into(),
                    trailing: Trailing::None,
                    destructive: true,
                    on_select: Some(Box::new(|this, _, cx| this.delete_current_note(cx))),
                    submenu: None,
                },
            ],
        };
        if !show_upload {
            let start = spec
                .entries
                .iter()
                .position(|entry| matches!(entry, Entry::Item { label, .. } if label.as_ref() == "Upload audio"))
                .expect("upload entries present");
            spec.entries.drain(start..start + 2);
        }
        spec
    }

    /// `FolderPickerSubmenu`: every folder in use plus "No folder", the
    /// current one checked; picking runs `updateSession({ folder_id })`.
    fn folder_submenu_entries(&self) -> Vec<Entry> {
        let current = match &self.note {
            super::Note::Ready { preview, .. } => {
                crate::timeline::normalize_folder_path(&preview.session.folder_id)
                    .unwrap_or_default()
            }
            _ => String::new(),
        };
        let mut folders: Vec<String> = self
            .session_rows
            .iter()
            .filter_map(|row| crate::timeline::normalize_folder_path(&row.folder_id))
            .filter(|folder| !folder.is_empty())
            .collect();
        folders.sort();
        folders.dedup();
        let mut entries: Vec<Entry> = folders
            .into_iter()
            .map(|folder| {
                let checked = folder == current;
                let target = folder.clone();
                Entry::Item {
                    icon: Some("folder"),
                    dim_icon: true,
                    label: folder.into(),
                    trailing: Trailing::Check(checked),
                    destructive: false,
                    on_select: Some(Box::new(move |this, _, cx| {
                        this.move_current_note_to_folder(target.clone(), cx)
                    })),
                    submenu: None,
                }
            })
            .collect();
        if !entries.is_empty() {
            entries.push(Entry::Separator);
        }
        entries.push(Entry::Item {
            icon: None,
            dim_icon: false,
            label: "No folder".into(),
            trailing: Trailing::Check(current.is_empty()),
            destructive: false,
            on_select: Some(Box::new(|this, _, cx| {
                this.move_current_note_to_folder(String::new(), cx)
            })),
            submenu: None,
        });
        entries
    }

    fn move_current_note_to_folder(&mut self, folder: String, cx: &mut Context<Self>) {
        let Some(session_id) = self.selected.clone() else {
            return;
        };
        let task = self.store.update_folder(session_id.clone(), folder);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    this.reload_note(session_id, cx);
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to move note"),
            Err(error) => tracing::error!(%error, "failed to move note"),
        })
        .detach();
    }

    /// `DropdownMenuContent variant="app" align="end" className="w-56"` under
    /// the `…` trigger; measured against the app, the chrome starts 81px from
    /// the window top with its right edge 8px in from the window's.
    pub(super) fn render_overflow_menu(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if !self.overflow_open {
            return None;
        }
        let viewport = window.viewport_size();
        let mut spec = self.overflow_spec();
        if self.is_standalone() {
            // `{!standaloneWindow && <Open in New Window>}`
            spec.entries.retain(
                |entry| !matches!(entry, Entry::Item { label, .. } if label.as_ref() == "Open in New Window"),
            );
        }
        Some(self.render_app_menu(
            spec,
            point(viewport.width - px(8.0), px(81.0)),
            Align::End,
            window,
            cx,
        ))
    }

    /// `useDeleteSession`: tombstone the note, close its tab, and offer undo
    /// for five seconds.
    fn delete_current_note(&mut self, cx: &mut Context<Self>) {
        let Some(session_id) = self.selected.clone() else {
            return;
        };
        let title = match &self.note {
            super::Note::Ready { preview, .. } => preview.session.title.clone(),
            _ => String::new(),
        };
        if let Some(editor) = self.editor.take() {
            editor.update(cx, |editor, _| {
                editor.take_pending();
            });
        }
        let task = self.store.soft_delete_session(session_id.clone());
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(Some(tombstone))) => {
                this.update(cx, |this, cx| {
                    this.close_note_windows(&session_id, cx);
                    this.tabs.retain(|id| id != &session_id);
                    this.close_selected_note(cx);
                    this.pending_deletions.push(PendingDeletion {
                        session_id,
                        title,
                        tombstone,
                        added_at: Instant::now(),
                    });
                    this.schedule_undo_expiry(cx);
                    this.reload_sessions(cx);
                })
                .ok();
            }
            Ok(Ok(None)) => {}
            Ok(Err(error)) => tracing::error!(%error, "failed to delete note"),
            Err(error) => tracing::error!(%error, "failed to delete note"),
        })
        .detach();
    }

    /// Clears the selection like closing the active tab does: the next tab
    /// in the list becomes active, or the empty view shows.
    fn close_selected_note(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.note = super::Note::Empty;
        if let Some(next) = self.tabs.last().cloned() {
            self.select(next, cx);
        }
        cx.notify();
    }

    /// Repaints the draining gauge until the undo window closes.
    fn schedule_undo_expiry(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let done = this
                    .update(cx, |this, cx| {
                        this.pending_deletions
                            .retain(|deletion| deletion.added_at.elapsed() < UNDO_TIMEOUT);
                        cx.notify();
                        this.pending_deletions.is_empty()
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    /// `restoreDeletedSession`: lift the tombstone and reopen the note.
    fn undo_delete(&mut self, session_id: String, cx: &mut Context<Self>) {
        let Some(index) = self
            .pending_deletions
            .iter()
            .position(|deletion| deletion.session_id == session_id)
        else {
            return;
        };
        let deletion = self.pending_deletions.remove(index);
        cx.notify();
        let task = self
            .store
            .restore_session(deletion.session_id.clone(), deletion.tombstone);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    this.select(deletion.session_id, cx);
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "could not restore deleted note"),
            Err(error) => tracing::error!(%error, "could not restore deleted note"),
        })
        .detach();
    }

    /// `UndoDeleteToast`: "<title> deleted" with an Undo action and a gauge
    /// draining over the five-second window, in the toast host's slot.
    pub(super) fn render_undo_toast(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let deletion = self.pending_deletions.last()?;
        let theme = self.theme;
        let text = theme.toast_text;
        let title = if deletion.title.trim().is_empty() {
            "Untitled".to_string()
        } else {
            deletion.title.clone()
        };
        let progress =
            1.0 - (deletion.added_at.elapsed().as_secs_f32() / UNDO_TIMEOUT.as_secs_f32()).min(1.0);
        let session_id = deletion.session_id.clone();
        Some(
            div()
                .id("undo-delete-toast")
                .absolute()
                .right(px(32.0))
                .bottom(px(32.0))
                .w(px(300.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .p(px(16.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(theme.toast_border)
                .bg(theme.toast_background)
                .overflow_hidden()
                .shadow(vec![BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.1),
                    offset: point(px(0.0), px(4.0)),
                    blur_radius: px(12.0),
                    spread_radius: px(0.0),
                }])
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.0))
                        .line_height(px(19.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(text)
                        .child(SharedString::from(format!("{title} deleted"))),
                )
                .child(
                    div()
                        .id("undo-delete-action")
                        .ml_auto()
                        .h(px(24.0))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded(px(4.0))
                        .bg(text)
                        .text_color(theme.toast_background)
                        .text_size(px(12.0))
                        .line_height(px(24.0))
                        .cursor_pointer()
                        .hover(move |style| style.bg(alpha(text, 0.9)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.undo_delete(session_id.clone(), cx);
                        }))
                        .child("Undo"),
                )
                // `bg-muted absolute inset-x-0 bottom-0 h-1` with the `bg-primary` gauge.
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(4.0))
                        .bg(theme.muted)
                        .child(div().h_full().w(px(300.0 * progress)).bg(theme.primary)),
                )
                .into_any_element(),
        )
    }
}

/// `find_session_dir`: the folder for a session under `<vault>/sessions`,
/// searching one level of subfolders, falling back to the direct child.
pub(crate) fn find_session_dir(sessions_base: &std::path::Path, session_id: &str) -> PathBuf {
    if let Ok(entries) = std::fs::read_dir(sessions_base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == session_id) {
                return path;
            }
            if path.is_dir() {
                let nested = path.join(session_id);
                if nested.is_dir() {
                    return nested;
                }
            }
        }
    }
    sessions_base.join(session_id)
}
