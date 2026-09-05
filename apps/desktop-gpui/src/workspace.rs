mod document_view;
mod note;
mod sidebar;
mod title_bar;

use std::sync::Arc;

use chrono::{Local, Utc};
use gpui::{
    Context, Decorations, ListAlignment, ListState, MouseButton, Render, SharedString, Window, div,
    prelude::*, px,
};

use crate::db::{NotePreview, Store};
use crate::theme::Theme;
use crate::timeline::{self, Timeline};
use crate::ui::TailwindText as _;

/// `LEFT_SIDEBAR_DEFAULT_WIDTH_PX` in `apps/desktop/src/main/left-sidebar-panel.ts`.
const SIDEBAR_WIDTH: f32 = 200.0;
const RESIZE_EDGE: f32 = 5.0;

enum Sessions {
    Loading,
    Ready(Timeline),
    Failed(String),
}

/// One line of the sidebar list; buckets and their rows share one flat list
/// so the variable-height `list` element can virtualize them together.
#[derive(Clone)]
enum SidebarRow {
    /// `data-sidebar-timeline-top-spacer`: room for the floating chips.
    Spacer,
    Header {
        bucket: usize,
    },
    Session {
        bucket: usize,
        item: usize,
    },
}

/// `computeCurrentNoteTab` with no remembered tab and no live session.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NoteTab {
    Memo,
    Enhanced(String),
}

enum Note {
    Empty,
    Loading,
    Ready { preview: NotePreview, tab: NoteTab },
    Failed(String),
}

pub struct Workspace {
    store: Arc<Store>,
    theme: Theme,
    font_family: Option<SharedString>,
    mono_font_family: Option<SharedString>,
    sessions: Sessions,
    rows: Vec<SidebarRow>,
    list_state: ListState,
    selected: Option<String>,
    note: Note,
    sidebar_expanded: bool,
    /// Id of the chrome button under the pointer, so icons can take the
    /// `hover:text-foreground` colour their container cannot pass down.
    hovered: Option<&'static str>,
}

impl Workspace {
    pub fn new(store: Arc<Store>, cx: &mut Context<Self>) -> Self {
        let font_family = crate::theme::ui_font_family(cx.text_system()).map(SharedString::from);
        let mono_font_family =
            crate::theme::mono_font_family(cx.text_system()).map(SharedString::from);
        let mut this = Self {
            store,
            theme: Theme::light(),
            font_family,
            mono_font_family,
            sessions: Sessions::Loading,
            rows: Vec::new(),
            list_state: ListState::new(0, ListAlignment::Top, px(400.0)),
            selected: None,
            note: Note::Empty,
            sidebar_expanded: true,
            hovered: None,
        };
        this.reload_sessions(cx);
        this.watch_changes(cx);
        this
    }

    /// Re-reads the list and the open note whenever the Tauri app commits.
    fn watch_changes(&self, cx: &mut Context<Self>) {
        let mut changes = self.store.changes();
        cx.spawn(async move |this, cx| {
            while changes.changed().await.is_ok() {
                let keep_going = this
                    .update(cx, |this, cx| {
                        this.reload_sessions(cx);
                        if let Some(selected) = this.selected.clone() {
                            this.reload_note(selected, cx);
                        }
                    })
                    .is_ok();
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    fn reload_sessions(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.sessions, Sessions::Ready(_)) {
            self.sessions = Sessions::Loading;
        }
        let task = self.store.list_timeline();
        cx.spawn(async move |this, cx| {
            let result = match task.await {
                Ok(Ok((rows, events))) => {
                    Sessions::Ready(timeline::build(&rows, &events, Utc::now(), &Local))
                }
                Ok(Err(error)) => Sessions::Failed(error.to_string()),
                Err(error) => Sessions::Failed(error.to_string()),
            };
            this.update(cx, |this, cx| {
                this.sessions = result;
                this.rebuild_rows();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        if let Sessions::Ready(timeline) = &self.sessions {
            if timeline.has_more_future_items {
                self.rows.push(SidebarRow::Spacer);
            }
            for (bucket_ix, bucket) in timeline.buckets.iter().enumerate() {
                self.rows.push(SidebarRow::Header { bucket: bucket_ix });
                for item_ix in 0..bucket.items.len() {
                    self.rows.push(SidebarRow::Session {
                        bucket: bucket_ix,
                        item: item_ix,
                    });
                }
            }
        }
        self.list_state.reset(self.rows.len());
    }

    fn select(&mut self, session_id: String, cx: &mut Context<Self>) {
        if self.selected.as_deref() == Some(session_id.as_str()) {
            return;
        }
        self.selected = Some(session_id.clone());
        self.note = Note::Loading;
        cx.notify();
        self.reload_note(session_id, cx);
    }

    fn reload_note(&mut self, session_id: String, cx: &mut Context<Self>) {
        let task = self.store.load_note(session_id.clone());
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                // A newer selection may have raced this load; keep the latest.
                if this.selected.as_deref() != Some(session_id.as_str()) {
                    return;
                }
                this.note = match result {
                    Ok(Ok(Some(preview))) => {
                        let tab = this.current_tab_for(&preview);
                        Note::Ready { preview, tab }
                    }
                    Ok(Ok(None)) => Note::Failed("This note no longer exists.".to_string()),
                    Ok(Err(error)) => Note::Failed(error.to_string()),
                    Err(error) => Note::Failed(error.to_string()),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `computeCurrentNoteTab`: keep the remembered tab while it still exists,
    /// otherwise the first enhanced note, otherwise the memo.
    fn current_tab_for(&self, preview: &NotePreview) -> NoteTab {
        let first_enhanced = preview
            .enhanced
            .first()
            .map(|doc| NoteTab::Enhanced(doc.id.clone()));
        match &self.note {
            Note::Ready {
                tab: NoteTab::Memo, ..
            } => NoteTab::Memo,
            Note::Ready {
                tab: NoteTab::Enhanced(id),
                ..
            } if preview.enhanced.iter().any(|doc| &doc.id == id) => NoteTab::Enhanced(id.clone()),
            _ => first_enhanced.unwrap_or(NoteTab::Memo),
        }
    }

    fn set_tab(&mut self, tab: NoteTab, cx: &mut Context<Self>) {
        if let Note::Ready { tab: current, .. } = &mut self.note
            && *current != tab
        {
            *current = tab;
            cx.notify();
        }
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_expanded = !self.sidebar_expanded;
        cx.notify();
    }

    fn set_hovered(&mut self, id: &'static str, hovered: bool, cx: &mut Context<Self>) {
        let next = hovered.then_some(id);
        if self.hovered != next && (hovered || self.hovered == Some(id)) {
            self.hovered = next;
            cx.notify();
        }
    }

    /// Icon colour for a chrome button: muted, or foreground while hovered.
    fn chrome_icon_color(&self, id: &'static str) -> gpui::Rgba {
        if self.hovered == Some(id) {
            self.theme.foreground
        } else {
            self.theme.muted_foreground
        }
    }

    /// `chrome_button` wired to hover tracking.
    fn tracked_chrome_button(
        &self,
        id: &'static str,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        crate::ui::chrome_button(id, self.theme)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.set_hovered(id, *hovered, cx);
            }))
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let client_decorations = matches!(window.window_decorations(), Decorations::Client { .. });

        // `ShellFrame`: title bar (Windows/Linux) above the `shell-scaffold`
        // row of sidebar + main surface, all on `bg-background`.
        div()
            .id("window")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .tw_text_sm()
            .when_some(self.font_family.clone(), |root, family| {
                root.font_family(family)
            })
            .when(client_decorations, |root| {
                root.on_mouse_down(MouseButton::Left, |event, window, _cx| {
                    let size = window.window_bounds().get_bounds().size;
                    if let Some(edge) =
                        title_bar::resize_edge(event.position, px(RESIZE_EDGE), size)
                    {
                        window.start_window_resize(edge);
                    }
                })
            })
            .when(title_bar::uses_windows_style_title_bar(), |root| {
                root.child(self.render_title_bar(window, cx))
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap_1()
                    .pl_1()
                    .when(self.sidebar_expanded, |shell| {
                        shell.child(self.render_sidebar(cx))
                    })
                    .child(self.render_main_surface(window, cx)),
            )
    }
}
