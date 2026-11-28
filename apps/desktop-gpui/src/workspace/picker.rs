//! Shipping's open-note dialog (`apps/desktop/src/shared/open-note-dialog.tsx`), opened by the
//! sidebar search button and Mod+K. `search` decides what it lists; this view draws it and owns
//! the notes snapshot, the persisted recently opened notes and the cmdk-style selection.

use std::{collections::VecDeque, sync::Arc, time::Duration};

use anlg_desktop_identity::store_json::StoreJson;
use desktop_runtime::{
    CancellationToken, Generation, Reply, RuntimeHandle, ServiceError, SessionId,
};
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, Font, KeyDownEvent,
    ListAlignment, ListState, MouseButton, Pixels, Render, SharedString, Subscription, Task,
    TextRun, Window, div, list, prelude::*, px, svg,
};
use serde_json::json;

use super::{
    mutations::failure,
    navigation::Route,
    search::{self, MAX_RECENT, Move, Note, Row},
};
use crate::shipping_store::{DESKTOP, RECENTLY_OPENED_SESSIONS};
use crate::ui::a11y::Accessible;
use crate::{
    product::account,
    ui::{
        input::{InputEvent, TextInput},
        theme::{system_font, theme},
    },
};

/// Local notes newest first (ties keep shipping's `created_at DESC, id`), then the viewer's
/// shared notes that are not already a managed local note.
pub(super) const NOTES_QUERY: &str = "SELECT kind, id, title FROM (
    SELECT 'session' AS kind, id, substr(title,1,4096) AS title, created_at AS date, 0 AS source
    FROM sessions WHERE deleted_at IS NULL
    UNION ALL
    SELECT 'shared', c.share_id, substr(c.title,1,4096), c.published_at, 1
    FROM shared_session_cache c WHERE c.viewer_user_id = ?1
        AND NOT (c.manage_access = 1 AND EXISTS(
            SELECT 1 FROM sessions s WHERE s.id = c.session_id AND s.deleted_at IS NULL))
) ORDER BY julianday(date) DESC, source, date DESC, id";

/// A sandbox's list; a channel run uses shipping's `RecentlyOpenedSessions`.
const RECENT_KEY: &str = "gpui_recent_sessions";

#[derive(Clone)]
pub enum PickerEvent {
    /// The destination, and whether it opens in a new tab.
    Open(Route, bool),
    Dismiss,
}

pub struct NotePicker {
    runtime: RuntimeHandle,
    input: Entity<TextInput>,
    notes: Arc<[Note]>,
    recent: VecDeque<SessionId>,
    shipping: Option<Arc<StoreJson>>,
    /// Until the stored list is read, opened notes are kept in memory and not written back.
    restored: bool,
    viewer: Option<Arc<str>>,
    rows: Vec<Row>,
    selected: Option<usize>,
    list: ListState,
    /// Horizontal offset that centers the dialog over the main content instead of the window.
    offset: Pixels,
    generation: Generation,
    cancel: CancellationToken,
    search_task: Option<Task<()>>,
    _subscription: Subscription,
}

impl EventEmitter<PickerEvent> for NotePicker {}

impl Focusable for NotePicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl NotePicker {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new("Search notes and pages...", cx)
                .inline()
                .label("Search notes and pages")
        });
        let subscription = cx.subscribe(&input, |this, _, event, cx| {
            if let InputEvent::Changed = event {
                this.refilter(false, cx);
            }
        });
        let mut picker = Self {
            runtime,
            input,
            notes: Arc::from([]),
            recent: VecDeque::new(),
            shipping: crate::shipping_store::ShippingStore::get(cx),
            restored: false,
            viewer: None,
            rows: Vec::new(),
            selected: None,
            list: ListState::new(0, ListAlignment::Top, px(320.)),
            offset: px(0.),
            generation: Generation::default(),
            cancel: CancellationToken::new(),
            search_task: None,
            _subscription: subscription,
        };
        picker.refilter(false, cx);
        picker
    }

    pub fn set_viewer(&mut self, viewer: Option<Arc<str>>, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.generation.advance();
        self.viewer = viewer;
        self.notes = Arc::from([]);
        self.refilter(true, cx);
    }

    /// Records an opened note, like shipping's `addRecentlyOpened`.
    pub fn remember(&mut self, id: SessionId, cx: &mut Context<Self>) {
        if self.recent.front() == Some(&id) {
            return;
        }
        self.recent.retain(|existing| existing != &id);
        self.recent.push_front(id);
        self.recent.truncate(MAX_RECENT);
        if self.restored {
            self.save_recent(cx);
        }
    }

    pub fn open(&mut self, offset: Pixels, window: &mut Window, cx: &mut Context<Self>) {
        self.offset = offset;
        self.input
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.input.focus_handle(cx).focus(window, cx);
        self.refilter(false, cx);
        self.load(cx);
    }

    /// Reads the notes and, the first time, the stored recently opened notes.
    pub fn load(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        let generation = self.generation.advance();
        let viewer = self.viewer.clone();
        let notes = self
            .runtime
            .read(self.cancel.clone(), move |services| async move {
                let rows = services
                    .executor
                    .execute(NOTES_QUERY.into(), vec![json!(viewer)])
                    .await
                    .map_err(failure)?;
                Ok(rows
                    .iter()
                    .map(|row| {
                        let id = row["id"].as_str().unwrap_or_default();
                        Note {
                            route: if row["kind"] == "shared" {
                                Route::SharedSession(id.into())
                            } else {
                                Route::Session(SessionId(id.into()))
                            },
                            title: row["title"]
                                .as_str()
                                .filter(|title| !title.is_empty())
                                .unwrap_or("Untitled")
                                .into(),
                        }
                    })
                    .collect::<Arc<[Note]>>())
            });
        let stored = (!self.restored)
            .then(|| load_recent(&self.runtime, self.shipping.clone(), self.cancel.clone()));
        cx.spawn(async move |this, cx| {
            let notes = match notes {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let stored = match stored {
                Some(Ok(reply)) => Some(reply.receive().await),
                Some(Err(error)) => Some(Err(error)),
                None => None,
            };
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    return;
                }
                match stored {
                    Some(Ok(stored)) if !this.restored => {
                        let opened = this.recent.len();
                        this.recent.extend(stored);
                        let mut seen = std::collections::HashSet::new();
                        this.recent.retain(|id| seen.insert(id.clone()));
                        this.recent.truncate(MAX_RECENT);
                        this.restored = true;
                        if opened > 0 {
                            this.save_recent(cx);
                        }
                    }
                    Some(Err(ServiceError::Cancelled)) | Some(Ok(_)) | None => {}
                    Some(Err(error)) => {
                        tracing::warn!(%error, "recently opened notes could not be read")
                    }
                }
                match notes {
                    Ok(notes) => this.notes = notes,
                    Err(ServiceError::Cancelled) => return,
                    Err(error) => tracing::warn!(%error, "open-note dialog could not read notes"),
                }
                this.refilter(true, cx);
            });
        })
        .detach();
    }

    fn save_recent(&self, cx: &mut Context<Self>) {
        let reply = save_recent(
            &self.runtime,
            self.shipping.clone(),
            self.recent.iter().cloned().collect(),
        );
        cx.background_spawn(async move {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                tracing::warn!(%error, "recently opened notes were not saved");
            }
        })
        .detach();
    }

    /// Rebuilds the rows. A new query selects the first item, as cmdk does; a reload keeps the
    /// selected item when it is still listed.
    fn refilter(&mut self, keep: bool, cx: &mut Context<Self>) {
        let recent = self.recent.iter().cloned().collect::<Vec<_>>();
        let rows = search::rows(&self.notes, &recent, &self.input.read(cx).buffer.text);
        let key = self
            .selected
            .filter(|_| keep)
            .and_then(|index| self.rows.get(index))
            .and_then(Row::key);
        let kept = key.and_then(|key| rows.iter().position(|row| row.key().as_ref() == Some(&key)));
        let old = self.rows.len();
        self.rows = rows;
        self.selected = kept.or_else(|| search::step(&self.rows, None, Move::Next));
        if keep {
            self.list.splice(0..old, self.rows.len());
        } else {
            self.list.reset(self.rows.len());
            let has_query = !self.input.read(cx).buffer.text.trim().is_empty();
            if has_query {
                self.search_task = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        if this.input.read(cx).buffer.text.trim().is_empty() {
                            return;
                        }
                        let mut result_count = 0usize;
                        let mut entity_types = std::collections::BTreeSet::new();
                        for row in &this.rows {
                            match row {
                                Row::Page(_) => {
                                    result_count += 1;
                                    entity_types.insert("page");
                                }
                                Row::Note { note, .. } => {
                                    result_count += 1;
                                    entity_types.insert(match note.route {
                                        Route::SharedSession(_) => "shared_session",
                                        _ => "session",
                                    });
                                }
                                Row::Heading { .. } | Row::Gap => {}
                            }
                        }
                        let entity_types: Vec<&'static str> = entity_types.into_iter().collect();
                        crate::telemetry::track(
                            "search_performed",
                            &[
                                ("entry_point", json!("open_note_dialog")),
                                ("result_count", json!(result_count)),
                                ("entity_types", json!(entity_types)),
                            ],
                        );
                    });
                }));
            } else {
                self.search_task = None;
            }
        }
        cx.notify();
    }

    fn select(&mut self, movement: Move) {
        self.selected = search::step(&self.rows, self.selected, movement);
        let Some(selected) = self.selected else {
            return;
        };
        // cmdk also reveals the group heading above a group's first item.
        if let Some(Row::Heading { .. }) = selected.checked_sub(1).map(|index| &self.rows[index]) {
            self.list.scroll_to_reveal_item(selected - 1);
        }
        self.list.scroll_to_reveal_item(selected);
    }

    fn choose(&mut self, cx: &mut Context<Self>) {
        if let Some(row) = self.selected.and_then(|index| self.rows.get(index))
            && let Some((route, new_tab)) = row.destination()
        {
            let result_type = match row {
                Row::Page(_) => "page",
                Row::Note { note, .. } => match note.route {
                    Route::SharedSession(_) => "shared_session",
                    _ => "session",
                },
                Row::Heading { .. } | Row::Gap => "unknown",
            };
            let had_query = !self.input.read(cx).buffer.text.trim().is_empty();
            crate::telemetry::track(
                "search_result_opened",
                &[
                    ("entry_point", json!("open_note_dialog")),
                    ("result_type", json!(result_type)),
                    ("had_query", json!(had_query)),
                ],
            );
            cx.emit(PickerEvent::Open(route, new_tab));
        }
    }

    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.input.read(cx).buffer.marked.is_some() {
            return;
        }
        let modifiers = event.keystroke.modifiers;
        let (down, up) = if modifiers.platform {
            (Move::Last, Move::First)
        } else if modifiers.alt {
            (Move::NextGroup, Move::PreviousGroup)
        } else {
            (Move::Next, Move::Previous)
        };
        match event.keystroke.key.as_str() {
            "escape" => cx.emit(PickerEvent::Dismiss),
            "enter" => self.choose(cx),
            "down" => self.select(down),
            "up" => self.select(up),
            "n" | "j" if modifiers.control => self.select(Move::Next),
            "p" | "k" if modifiers.control => self.select(Move::Previous),
            "home" => self.select(Move::First),
            "end" => self.select(Move::Last),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn render_row(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let muted = colors.muted_foreground;
        let (icon, title, hint, locked): (&'static str, SharedString, _, _) =
            match &self.rows[index] {
                Row::Heading { label, separator } => {
                    return div()
                        .id(index)
                        .a11y_heading(*label, 3)
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .when(*separator, |view| {
                            view.child(div().mx_2().h(px(1.)).bg(colors.accent))
                        })
                        .child(
                            div()
                                .px_2()
                                .py(px(6.))
                                .flex()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .font_weight(crate::ui::theme::MEDIUM)
                                .text_color(muted)
                                .children(tracked(&label.to_uppercase(), 12., window, cx)),
                        )
                        .into_any_element();
                }
                Row::Gap => return div().h(px(6.)).into_any_element(),
                Row::Page(page) => (
                    page.icon,
                    page.label.into(),
                    search::hint(*page),
                    account::locked(page.id, page.requires_pro, &account::current(cx)),
                ),
                Row::Note { note, .. } => (
                    if matches!(note.route, Route::SharedSession(_)) {
                        "workspace/UsersIcon.svg"
                    } else {
                        "workspace/FileTextIcon.svg"
                    },
                    note.title.clone().into(),
                    None,
                    false,
                ),
            };
        div()
            .id(index)
            .a11y_option(
                if locked {
                    SharedString::from(format!("{title}, Requires Anarlog Pro"))
                } else {
                    title.clone()
                },
                self.selected == Some(index),
            )
            .w_full()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .py(px(10.))
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(muted)
            .cursor_pointer()
            .when(self.selected == Some(index), |view| {
                view.bg(colors.accent.opacity(0.6))
            })
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.selected != Some(index) {
                    this.selected = Some(index);
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = Some(index);
                this.choose(cx);
            }))
            .child(
                svg()
                    .path(icon)
                    .size(px(16.))
                    .flex_shrink_0()
                    .text_color(muted),
            )
            .child(div().min_w_0().truncate().child(title))
            .when_some(hint, |view, hint| {
                view.child(
                    div()
                        .ml_auto()
                        .flex_shrink_0()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .child(hint),
                )
            })
            .when(locked, |view| {
                view.child(
                    svg()
                        .path("product/LockPasswordIcon.svg")
                        .size(px(14.))
                        .flex_shrink_0()
                        .text_color(muted),
                )
            })
            .into_any_element()
    }
}

/// `tracking-wider` (0.05em): GPUI text runs have no letter spacing, so each letter is laid
/// out at its own advance plus the tracking.
fn tracked(text: &str, size: f32, window: &Window, cx: &App) -> Vec<gpui::Div> {
    let font = Font {
        weight: crate::ui::theme::MEDIUM,
        ..gpui::font(system_font(cx))
    };
    text.chars()
        .map(|letter| {
            let letter = SharedString::from(letter.to_string());
            let run = TextRun {
                len: letter.len(),
                font: font.clone(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let advance = window
                .text_system()
                .shape_line(letter.clone(), px(size), &[run], None)
                .width;
            div().flex_none().w(advance + px(size * 0.05)).child(letter)
        })
        .collect()
}

/// Shipping's row heights: `py-1.5` 16px headings (plus the separator's 1px and `gap-3`),
/// `py-2.5` 20px items and the `pb-1.5` group gap.
fn row_height(row: &Row) -> f32 {
    match row {
        Row::Heading { separator, .. } => 28. + if *separator { 13. } else { 0. },
        Row::Page(_) | Row::Note { .. } => 40.,
        Row::Gap => 6.,
    }
}

/// The stored recently opened notes, newest first. Like shipping, malformed storage reads as
/// empty and is replaced by the next save. A channel run shares shipping's list in
/// `store.json`; a sandbox keeps it in its own settings.
pub(super) fn load_recent(
    runtime: &RuntimeHandle,
    shipping: Option<Arc<StoreJson>>,
    cancel: CancellationToken,
) -> desktop_runtime::Result<Reply<Vec<SessionId>>> {
    runtime.read(cancel, |services| async move {
        let raw = match shipping {
            Some(store) => crate::shipping_store::read(store, DESKTOP, RECENTLY_OPENED_SESSIONS)
                .await?
                .and_then(|value| value.as_str().map(str::to_owned)),
            None => services
                .executor
                .execute(
                    "SELECT value_json FROM app_settings WHERE id = ?".into(),
                    vec![json!(RECENT_KEY)],
                )
                .await
                .map_err(failure)?
                .first()
                .and_then(|row| row["value_json"].as_str().map(str::to_owned)),
        };
        Ok(raw
            .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|id| !id.is_empty())
            .take(MAX_RECENT)
            .map(|id| SessionId(id.into()))
            .collect())
    })
}

pub(super) fn save_recent(
    runtime: &RuntimeHandle,
    shipping: Option<Arc<StoreJson>>,
    recent: Vec<SessionId>,
) -> desktop_runtime::Result<Reply<()>> {
    let value = json!(recent.iter().map(|id| id.0.as_ref()).collect::<Vec<_>>()).to_string();
    runtime.submit(move |services| async move {
        if let Some(store) = shipping {
            let value = json!(value);
            return crate::shipping_store::write(store, DESKTOP, RECENTLY_OPENED_SESSIONS, value)
                .await;
        }
        services
            .executor
            .execute(
                "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at".into(),
                vec![json!(RECENT_KEY), json!(value)],
            )
            .await
            .map(|_| ())
            .map_err(failure)
    })
}

impl Drop for NotePicker {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Render for NotePicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let muted = colors.muted_foreground;
        let viewport = window.viewport_size();
        let width = viewport.width.min(px(512.));
        let top = viewport.height * 0.15;
        let header = div()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(colors.border.opacity(0.6))
            .px_4()
            .py_3()
            .child(
                svg()
                    .path("Search01Icon.svg")
                    .size(px(16.))
                    .flex_shrink_0()
                    .text_color(muted),
            )
            // The shared input is 24px tall; shipping's line is 20px.
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .child(self.input.clone()),
            )
            .child(
                div()
                    .id("close-open-note-dialog")
                    .a11y_button("Close")
                    .size(px(20.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(colors.accent.opacity(0.8))
                    .child(
                        svg()
                            .path("product/XIcon.svg")
                            .size(px(12.))
                            .text_color(muted),
                    )
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Dismiss))),
            );
        let results = if self.rows.is_empty() {
            div()
                .p_2()
                .child(
                    div()
                        .id("open-note-empty")
                        .a11y_status("No results found.", false)
                        .py_6()
                        .text_center()
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(muted)
                        .child("No results found."),
                )
                .into_any_element()
        } else {
            // `max-h-80` around the `p-2` list.
            let height = self.rows.iter().map(row_height).sum::<f32>() + 16.;
            div()
                .id("open-note-results")
                .a11y_group(gpui::Role::ListBox, "Notes and pages")
                .w_full()
                .child(
                    list(
                        self.list.clone(),
                        cx.processor(|this, index: usize, window, cx| {
                            this.render_row(index, window, cx)
                        }),
                    )
                    .w_full()
                    .h(px(height.min(320.)))
                    .p_2(),
                )
                .into_any_element()
        };
        // The input keeps focus while the arrows move the highlight, like shipping's combobox.
        // GPUI follows an active descendant only inside the focused element, so a polite live
        // region reads the highlighted row instead.
        let highlighted = self.selected.and_then(|index| match &self.rows[index] {
            Row::Page(page) => Some(SharedString::from(page.label)),
            Row::Note { note, .. } => Some(SharedString::from(note.title.clone())),
            _ => None,
        });
        div()
            .id("open-note-dialog")
            .a11y_dialog("Search notes and pages...")
            .absolute()
            .inset_0()
            .occlude()
            .bg(gpui::black().opacity(0.2))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Dismiss)),
            )
            .capture_key_down(cx.listener(Self::key))
            // Shipping keeps the top of the overlay as a window drag region that does not close.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(top)
                    .on_mouse_down(MouseButton::Left, |_, window, cx| {
                        cx.stop_propagation();
                        window.start_window_move();
                    }),
            )
            .child(
                div()
                    .absolute()
                    .top(top)
                    .left((viewport.width - width) / 2. + self.offset)
                    .w(width)
                    .px_4()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .border_1()
                            .border_color(colors.border.opacity(0.8))
                            .bg(colors.background)
                            .shadow_2xl()
                            .child(header)
                            .child(results)
                            .when_some(highlighted, |view, title| {
                                view.child(
                                    div().id("open-note-highlight").a11y_status(title, false),
                                )
                            }),
                    ),
            )
    }
}
