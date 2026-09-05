//! `apps/desktop/src/session/components/outer-header/overflow`: the `…` menu
//! of the note header and the Delete flow with its undo toast
//! (`useDeleteSession` + `undo-delete-toast.tsx`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, BoxShadow, ClickEvent, Context, Corner, MouseButton, MouseDownEvent, SharedString,
    Window, anchored, deferred, div, hsla, point, prelude::*, px,
};

use super::Workspace;
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// `UNDO_TIMEOUT_MS`
pub(super) const UNDO_TIMEOUT: Duration = Duration::from_millis(5000);

/// A soft-deleted note the user can still bring back.
pub(crate) struct PendingDeletion {
    pub session_id: String,
    pub title: String,
    pub tombstone: String,
    pub added_at: Instant,
}

enum Item {
    Entry {
        icon: &'static str,
        label: &'static str,
        submenu: bool,
        destructive: bool,
        action: ItemAction,
    },
    Separator,
}

#[derive(Clone, Copy)]
enum ItemAction {
    None,
    Delete,
    ShowInFolder,
}

impl Workspace {
    pub(crate) fn toggle_overflow_menu(&mut self, cx: &mut Context<Self>) {
        self.overflow_open = !self.overflow_open;
        cx.notify();
    }

    fn close_overflow_menu(&mut self, cx: &mut Context<Self>) {
        if self.overflow_open {
            self.overflow_open = false;
            cx.notify();
        }
    }

    /// The items the Tauri menu shows for a note without audio, transcript,
    /// or an active recording; lock is only offered where the OS app lock
    /// exists, which Linux lacks.
    fn overflow_items(&self) -> Vec<Item> {
        vec![
            Item::Entry {
                icon: "folder",
                label: "Folder",
                submenu: true,
                destructive: false,
                action: ItemAction::None,
            },
            Item::Entry {
                icon: "calendar-blank",
                label: "Meeting info",
                submenu: true,
                destructive: false,
                action: ItemAction::None,
            },
            Item::Separator,
            Item::Entry {
                icon: "file-arrow-down",
                label: "Export",
                submenu: false,
                destructive: false,
                action: ItemAction::None,
            },
            Item::Separator,
            Item::Entry {
                icon: "microphone",
                label: "Start listening",
                submenu: false,
                destructive: false,
                action: ItemAction::None,
            },
            Item::Separator,
            Item::Entry {
                icon: "app-window",
                label: "Open in New Window",
                submenu: false,
                destructive: false,
                action: ItemAction::None,
            },
            Item::Entry {
                icon: "folder-open",
                label: if cfg!(target_os = "macos") {
                    "Show in Finder"
                } else {
                    "Show in folder"
                },
                submenu: false,
                destructive: false,
                action: ItemAction::ShowInFolder,
            },
            Item::Entry {
                icon: "trash",
                label: "Delete",
                submenu: false,
                destructive: true,
                action: ItemAction::Delete,
            },
        ]
    }

    fn run_overflow_action(&mut self, action: ItemAction, cx: &mut Context<Self>) {
        match action {
            ItemAction::None => {}
            ItemAction::Delete => self.delete_current_note(cx),
            ItemAction::ShowInFolder => {
                if let Some(path) = self
                    .selected
                    .as_deref()
                    .map(|id| self.store.session_dir(id))
                {
                    cx.open_url(&format!("file://{}", path.display()));
                }
            }
        }
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

    /// `DropdownMenuContent variant="app" align="end" className="w-56"`: a
    /// `rounded-[22px]` chrome with `p-0.5` around an `AppFloatingPanel`
    /// (`rounded-[20px] p-1.5`), anchored to the trigger's bottom-right.
    pub(super) fn render_overflow_menu(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if !self.overflow_open {
            return None;
        }
        let theme = self.theme;
        let viewport = window.viewport_size();
        let chrome = theme.floating_chrome;
        let panel_bg = theme.floating_panel;
        let border = theme.floating_border;
        let red_600 = theme.delete_text;
        let red_700 = theme.delete_hover_text;
        let red_50 = theme.delete_hover_background;

        let items = self
            .overflow_items()
            .into_iter()
            .enumerate()
            .map(|(index, item)| match item {
                Item::Separator => div()
                    .mx(px(-4.0))
                    .my_1()
                    .h(px(1.0))
                    .bg(theme.accent)
                    .into_any_element(),
                Item::Entry {
                    icon: glyph,
                    label,
                    submenu,
                    destructive,
                    action,
                } => {
                    let color = if destructive {
                        red_600
                    } else {
                        theme.foreground
                    };
                    div()
                        .id(("overflow-item", index))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(6.0))
                        .rounded(px(14.0))
                        .tw_text_sm()
                        .text_color(color)
                        .cursor_pointer()
                        .hover(move |style| {
                            if destructive {
                                style.bg(red_50).text_color(red_700)
                            } else {
                                style.bg(theme.accent)
                            }
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if submenu {
                                return;
                            }
                            this.close_overflow_menu(cx);
                            this.run_overflow_action(action, cx);
                        }))
                        .child(icon(glyph, px(16.0), color))
                        .child(SharedString::from(label))
                        .when(submenu, |item| {
                            item.child(div().ml_auto().child(icon("caret-right", px(16.0), color)))
                        })
                        .into_any_element()
                }
            });

        let panel = div()
            .id("overflow-menu")
            .occlude()
            .w(px(224.0))
            .p(px(2.0))
            .rounded(px(22.0))
            .border_1()
            .border_color(border)
            .bg(chrome)
            .shadow(vec![
                BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.1),
                    offset: point(px(0.0), px(10.0)),
                    blur_radius: px(15.0),
                    spread_radius: px(-3.0),
                },
                BoxShadow {
                    color: hsla(0.0, 0.0, 0.0, 0.1),
                    offset: point(px(0.0), px(4.0)),
                    blur_radius: px(6.0),
                    spread_radius: px(-4.0),
                },
            ])
            .on_mouse_down_out(
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_overflow_menu(cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(px(6.0))
                    .rounded(px(20.0))
                    .border_1()
                    .border_color(border)
                    .bg(panel_bg)
                    .children(items),
            );

        // `align="end"` puts the menu's right edge on the trigger's; measured
        // against the app, the panel's chrome starts 81px from the window top.
        let trigger_right = viewport.width - px(8.0);
        let trigger_bottom = px(77.0);
        Some(
            deferred(
                anchored()
                    .anchor(Corner::TopRight)
                    .position(point(trigger_right, trigger_bottom + px(4.0)))
                    .snap_to_window_with_margin(px(8.0))
                    .child(panel),
            )
            .with_priority(1)
            .into_any_element(),
        )
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
                        .line_height(px(19.5))
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
