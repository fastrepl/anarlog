mod document_view;
mod note;
mod open_note;
mod sidebar;
mod title_bar;

use std::sync::Arc;

use chrono::{Local, Utc};
use gpui::{
    Context, Decorations, FocusHandle, ListAlignment, ListState, MouseButton, MouseMoveEvent,
    MouseUpEvent, Pixels, Render, SharedString, Window, div, prelude::*, px,
};

use crate::actions;
use crate::db::{NotePreview, Store};
use crate::editor::{BodyEditor, EditorEvent};
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::Theme;
use crate::timeline::{self, Timeline};
use crate::ui::TailwindText as _;

/// `apps/desktop/src/main/left-sidebar-panel.ts`.
const SIDEBAR_DEFAULT_WIDTH: f32 = 200.0;
const SIDEBAR_MIN_WIDTH: f32 = 200.0;
const SIDEBAR_MAX_WIDTH: f32 = 360.0;
const RESIZE_EDGE: f32 = 5.0;

/// Which title bar menu is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Menu {
    File,
    Edit,
    View,
    Help,
}

struct SidebarDrag {
    start_x: Pixels,
    start_width: f32,
}

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
    Ready {
        preview: Box<NotePreview>,
        tab: NoteTab,
    },
    Failed(String),
}

pub struct Workspace {
    store: Arc<Store>,
    theme: Theme,
    focus_handle: FocusHandle,
    title_input: gpui::Entity<TextInput>,
    /// The memo editor for the selected session.
    editor: Option<gpui::Entity<BodyEditor>>,
    font_family: Option<SharedString>,
    mono_font_family: Option<SharedString>,
    sessions: Sessions,
    /// Every non-deleted session (`useSessionSummaries`), for the open-note dialog.
    session_rows: Vec<timeline::SessionRow>,
    rows: Vec<SidebarRow>,
    list_state: ListState,
    selected: Option<String>,
    note: Note,
    sidebar_expanded: bool,
    sidebar_width: f32,
    sidebar_drag: Option<SidebarDrag>,
    open_menu: Option<Menu>,
    open_note: Option<open_note::OpenNoteDialog>,
    /// `recentlyOpenedSessionIds`, newest first.
    recently_opened: Vec<String>,
    /// The tab store's session tabs, in order; the tab strip itself is not
    /// shown, but `openNew` vs `openCurrent` decide which note gets closed.
    tabs: Vec<String>,
    /// Id of the chrome button under the pointer, so icons can take the
    /// `hover:text-foreground` colour their container cannot pass down.
    hovered: Option<&'static str>,
}

impl Workspace {
    pub fn new(store: Arc<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let font_family = crate::theme::ui_font_family(cx.text_system()).map(SharedString::from);
        let mono_font_family =
            crate::theme::mono_font_family(cx.text_system()).map(SharedString::from);
        let theme = Theme::light();
        let title_input = cx.new(|cx| {
            TextInput::new(
                "Untitled",
                TextInputStyle {
                    text: theme.title,
                    placeholder: theme.muted_foreground,
                    selection: theme.selection,
                    underline_when_focused: true,
                },
                window,
                cx,
            )
        });
        cx.subscribe(&title_input, |this, _, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Committed {
                this.persist_title(cx);
            }
        })
        .detach();
        let mut this = Self {
            store,
            theme,
            focus_handle: cx.focus_handle(),
            title_input,
            editor: None,
            font_family,
            mono_font_family,
            sessions: Sessions::Loading,
            session_rows: Vec::new(),
            rows: Vec::new(),
            list_state: ListState::new(0, ListAlignment::Top, px(400.0)),
            selected: None,
            note: Note::Empty,
            sidebar_expanded: true,
            sidebar_width: SIDEBAR_DEFAULT_WIDTH,
            sidebar_drag: None,
            open_menu: None,
            open_note: None,
            recently_opened: Vec::new(),
            tabs: Vec::new(),
            hovered: None,
        };
        // Chips and the bottom fade depend on the scroll position.
        this.list_state
            .set_scroll_handler(cx.listener(|_, _: &gpui::ListScrollEvent, _, cx| cx.notify()));
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
            let (result, rows) = match task.await {
                Ok(Ok((rows, events))) => (
                    Sessions::Ready(timeline::build(&rows, &events, Utc::now(), &Local)),
                    rows,
                ),
                Ok(Err(error)) => (Sessions::Failed(error.to_string()), Vec::new()),
                Err(error) => (Sessions::Failed(error.to_string()), Vec::new()),
            };
            this.update(cx, |this, cx| {
                this.sessions = result;
                this.session_rows = rows;
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

    /// `openCurrent`: reuse the tab if the note is already open, otherwise
    /// replace the active slot, which closes the note that was there.
    fn select(&mut self, session_id: String, cx: &mut Context<Self>) {
        self.open_tab(session_id, false, cx);
    }

    /// `openNew`: the note opens in a new tab; the previous one stays open in
    /// the (invisible) tab list, so it is not closed or cleaned up.
    pub(crate) fn open_new(&mut self, session_id: String, cx: &mut Context<Self>) {
        self.open_tab(session_id, true, cx);
    }

    fn open_tab(&mut self, session_id: String, force_new: bool, cx: &mut Context<Self>) {
        // `addRecentlyOpened`
        self.recently_opened.retain(|id| id != &session_id);
        self.recently_opened.insert(0, session_id.clone());
        self.recently_opened
            .truncate(open_note::MAX_RECENT_SESSIONS);

        if self.selected.as_deref() == Some(session_id.as_str()) {
            return;
        }
        let previous = self.selected.replace(session_id.clone());
        let already_open = self.tabs.contains(&session_id);
        if !already_open {
            match previous
                .as_ref()
                .and_then(|id| self.tabs.iter().position(|t| t == id))
            {
                Some(slot) if !force_new => {
                    let closed = std::mem::replace(&mut self.tabs[slot], session_id.clone());
                    self.close_tab(closed, cx);
                }
                _ => self.tabs.push(session_id.clone()),
            }
        }
        self.note = Note::Loading;
        cx.notify();
        self.reload_note(session_id, cx);
    }

    /// `openCurrent` replaces the tab, so the previous note goes through the
    /// tab close handler: pending edits are written first, then an untouched
    /// note is soft-deleted.
    fn close_tab(&mut self, session_id: String, cx: &mut Context<Self>) {
        let pending = self
            .editor
            .as_ref()
            .filter(|editor| editor.read(cx).session_id == session_id)
            .and_then(|editor| editor.update(cx, |editor, _| editor.take_pending()));
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            if let Some(body) = pending
                && let Err(error) = store.update_memo(session_id.clone(), body).await
            {
                tracing::error!(%error, "failed to persist note");
            }
            match store.close_empty_session(session_id).await {
                Ok(Ok(true)) => {
                    this.update(cx, |this, cx| this.reload_sessions(cx)).ok();
                }
                Ok(Ok(false)) => {}
                Ok(Err(error)) => tracing::error!(%error, "session close cleanup"),
                Err(error) => tracing::error!(%error, "session close cleanup"),
            }
        })
        .detach();
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
                        // `title = draftTitle ?? storeTitle`
                        let title = preview.session.title.clone();
                        this.title_input.update(cx, |input, cx| {
                            if !input.is_dirty() {
                                input.set_text(title, cx);
                            }
                        });
                        this.sync_editor(&preview, cx);
                        Note::Ready {
                            preview: Box::new(preview),
                            tab,
                        }
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

    pub(crate) fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_expanded = !self.sidebar_expanded;
        cx.notify();
    }

    /// Keeps one `BodyEditor` per selected session, fed from the store unless
    /// it has unsaved edits.
    fn sync_editor(&mut self, preview: &NotePreview, cx: &mut Context<Self>) {
        let session_id = preview.session.id.clone();
        let body = preview.memo_body.clone();
        match &self.editor {
            Some(editor) if editor.read(cx).session_id == session_id => {
                editor.update(cx, |editor, cx| editor.replace_body(&body, cx));
            }
            _ => {
                if let Some(previous) = self.editor.take() {
                    previous.update(cx, |editor, cx| editor.flush(cx));
                }
                let editor = cx.new(|cx| BodyEditor::new(session_id, &body, cx));
                cx.subscribe(&editor, |this, editor, event: &EditorEvent, cx| {
                    let EditorEvent::Flush(json) = event;
                    let session_id = editor.read(cx).session_id.clone();
                    this.persist_memo(session_id, json.clone(), cx);
                })
                .detach();
                self.editor = Some(editor);
            }
        }
    }

    /// `updateSession({ raw_md })` from the editor's debounced flush.
    fn persist_memo(&mut self, session_id: String, body: String, cx: &mut Context<Self>) {
        let task = self.store.update_memo(session_id.clone(), body);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| {
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id, cx);
                    }
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to persist note"),
            Err(error) => tracing::error!(%error, "failed to persist note"),
        })
        .detach();
    }

    /// `persistTitle`: the title input's blur/Enter writes the draft.
    fn persist_title(&mut self, cx: &mut Context<Self>) {
        let Some(session_id) = self.selected.clone() else {
            return;
        };
        let title = self.title_input.read(cx).text().to_string();
        let task = self.store.update_title(session_id.clone(), title);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id, cx);
                    }
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to persist title"),
            Err(error) => tracing::error!(%error, "failed to persist title"),
        })
        .detach();
    }

    /// `useNewNote`: create the session, then open it as the current tab.
    pub(crate) fn new_note(&mut self, cx: &mut Context<Self>) {
        let task = self.store.create_note();
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(session_id)) => {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    this.open_new(session_id, cx);
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to create note"),
            Err(error) => tracing::error!(%error, "failed to create note"),
        })
        .detach();
    }

    /// Clicking a calendar event opens (creating if needed) its session.
    pub(crate) fn open_event(&mut self, event_id: String, cx: &mut Context<Self>) {
        let task = self.store.open_event_session(event_id);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(session_id)) => {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    this.select(session_id, cx);
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to open calendar event"),
            Err(error) => tracing::error!(%error, "failed to open calendar event"),
        })
        .detach();
    }

    fn set_menu(&mut self, menu: Option<Menu>, cx: &mut Context<Self>) {
        if self.open_menu != menu {
            self.open_menu = menu;
            cx.notify();
        }
    }

    /// `ResizableHandle` drag: clamp to the panel's min/max like the app.
    fn begin_sidebar_drag(&mut self, x: Pixels, cx: &mut Context<Self>) {
        self.sidebar_drag = Some(SidebarDrag {
            start_x: x,
            start_width: self.sidebar_width,
        });
        cx.notify();
    }

    fn update_sidebar_drag(&mut self, x: Pixels, cx: &mut Context<Self>) {
        if let Some(drag) = &self.sidebar_drag {
            let width = (drag.start_width + f32::from(x - drag.start_x))
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
            if width != self.sidebar_width {
                self.sidebar_width = width;
                cx.notify();
            }
        }
    }

    fn end_sidebar_drag(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_drag.take().is_some() {
            cx.notify();
        }
    }

    /// Switch between note views (`mod+alt+left/right`), in tab-strip order:
    /// enhanced notes first, then the memo.
    fn step_view(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Note::Ready { preview, tab } = &self.note else {
            return;
        };
        let mut tabs: Vec<NoteTab> = preview
            .enhanced
            .iter()
            .map(|doc| NoteTab::Enhanced(doc.id.clone()))
            .collect();
        tabs.push(NoteTab::Memo);
        let Some(index) = tabs.iter().position(|t| t == tab) else {
            return;
        };
        let next = (index as isize + delta).rem_euclid(tabs.len() as isize) as usize;
        let next = tabs[next].clone();
        self.set_tab(next, cx);
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
        let dragging = self.sidebar_drag.is_some();
        div()
            .id("window")
            .track_focus(&self.focus_handle)
            .key_context(actions::KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &actions::NewNote, _, cx| this.new_note(cx)))
            .on_action(
                cx.listener(|this, _: &actions::OpenNoteDialog, window, cx| {
                    this.open_note_dialog(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &actions::ToggleSidebar, _, cx| this.toggle_sidebar(cx)),
            )
            .on_action(cx.listener(|this, _: &actions::PreviousView, _, cx| this.step_view(-1, cx)))
            .on_action(cx.listener(|this, _: &actions::NextView, _, cx| this.step_view(1, cx)))
            .on_action(|_: &actions::ToggleFullscreen, window, _| window.toggle_fullscreen())
            .on_action(|_: &actions::CloseWindow, window, _| window.remove_window())
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .tw_text_sm()
            .when_some(self.font_family.clone(), |root, family| {
                root.font_family(family)
            })
            .when(dragging, |root| {
                root.cursor_col_resize()
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.update_sidebar_drag(event.position.x, cx);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_sidebar_drag(cx)),
                    )
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
            // `shell-scaffold`: `pl-1` only while the main surface has its left
            // chrome; collapsing the sidebar switches to `top-borderless`.
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(self.sidebar_expanded, |shell| {
                        shell
                            .pl_1()
                            .child(self.render_sidebar(cx))
                            .child(self.render_sidebar_handle(cx))
                    })
                    .when(!self.sidebar_expanded, |shell| shell.gap_1())
                    .child(self.render_main_surface(window, cx)),
            )
            .children(self.render_open_menu(window, cx))
            .children(self.render_open_note_dialog(window, cx))
    }
}
