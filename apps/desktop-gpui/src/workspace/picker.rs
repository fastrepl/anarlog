use std::{collections::VecDeque, sync::Arc};

use desktop_runtime::{CancellationToken, Generation, RuntimeHandle, ServiceError, SessionId};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, Render, SharedString,
    Subscription, UniformListScrollHandle, Window, div, prelude::*, px, uniform_list,
};
use serde_json::json;

use super::{navigation::Route, ports::escape_like};
use crate::ui::{
    input::{InputEvent, TextInput},
    theme::theme,
};

pub(super) const PICKER_QUERY: &str = "WITH recent AS (
    SELECT value AS id, key AS rank FROM json_each(?3)
), notes AS (
    SELECT 'session' AS kind, s.id, substr(s.title,1,4096) AS title, s.created_at AS date,
        COALESCE(recent.rank, 999) AS rank
    FROM sessions s LEFT JOIN recent ON s.id = recent.id WHERE s.deleted_at IS NULL
    UNION ALL
    SELECT 'shared' AS kind, c.share_id AS id, substr(c.title,1,4096) AS title, c.published_at AS date, 999 AS rank
    FROM shared_session_cache c WHERE c.viewer_user_id = ?2
        AND NOT (c.manage_access = 1 AND EXISTS(
            SELECT 1 FROM sessions s WHERE s.id = c.session_id AND s.deleted_at IS NULL))
) SELECT kind, id, title, rank FROM notes
WHERE title LIKE ?1 ESCAPE '\\'
ORDER BY rank, date DESC, kind, id LIMIT 201";

#[derive(Clone)]
pub enum PickerEvent {
    Open(Route),
    Dismiss,
}

#[derive(Clone)]
pub struct PickerRow {
    pub route: Route,
    pub title: Arc<str>,
    pub recent: bool,
}

pub struct NotePicker {
    runtime: RuntimeHandle,
    engine: super::search::SearchEngine,
    input: Entity<TextInput>,
    rows: Arc<[PickerRow]>,
    selected: usize,
    recent: VecDeque<SessionId>,
    viewer: Option<Arc<str>>,
    generation: Generation,
    cancel: CancellationToken,
    scroll: UniformListScrollHandle,
    message: String,
    loading: bool,
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
        let input = cx.new(|cx| TextInput::new("Open a note…", cx));
        let subscription = cx.subscribe(&input, |this, _, event, cx| match event {
            InputEvent::Changed if this.input.read(cx).buffer.marked.is_none() => this.load(cx),
            InputEvent::Submitted => this.choose(cx),
            _ => {}
        });
        Self {
            runtime,
            engine: super::search::SearchEngine::default(),
            input,
            rows: Arc::from([]),
            selected: 0,
            recent: VecDeque::new(),
            viewer: None,
            generation: Generation::default(),
            cancel: CancellationToken::new(),
            scroll: UniformListScrollHandle::new(),
            message: String::new(),
            loading: false,
            _subscription: subscription,
        }
    }

    pub fn set_viewer(&mut self, viewer: Option<Arc<str>>, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.generation.advance();
        self.viewer = viewer;
        self.rows = Arc::from([]);
        self.selected = 0;
        self.loading = false;
        self.message = "Account changed. Type to search.".into();
        cx.notify();
    }

    pub fn remember(&mut self, id: SessionId) {
        self.recent.retain(|existing| existing != &id);
        self.recent.push_front(id);
        self.recent.truncate(50);
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.input.focus_handle(cx).focus(window, cx);
        self.load(cx);
    }

    pub fn close(&mut self) {
        self.cancel.cancel();
        self.generation.advance();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        let generation = self.generation.advance();
        let query: Arc<str> = self.input.read(cx).buffer.text.as_str().into();
        let viewer = self.viewer.clone();
        let recent = self.recent.iter().take(5).cloned().collect::<Vec<_>>();
        if !query.trim().is_empty() {
            let reply = self
                .engine
                .query(&self.runtime, query, viewer, self.cancel.clone());
            self.loading = true;
            self.message = "Searching notes and transcripts…".into();
            cx.notify();
            cx.spawn(async move |this, cx| {
                let result = match reply { Ok(reply) => reply.receive().await, Err(error) => Err(error) };
                let _ = this.update(cx, |this, cx| {
                    if generation != this.generation { return; }
                    this.loading = false;
                    match result {
                        Ok(results) => {
                            this.rows = results.hits.iter().map(|hit| PickerRow { route: hit.route.clone(), title: hit.title.clone(), recent: false }).collect();
                            this.selected = 0;
                            this.message = if results.limited { "Bounded search: some content or results were omitted; refine the query." } else if this.rows.is_empty() { "No matching notes." } else { "↑ ↓ to select · Enter to open · Esc to close" }.into();
                        }
                        Err(error) => { this.rows = Arc::from([]); this.message = format!("Search failed: {error}"); }
                    }
                    cx.notify();
                });
            }).detach();
            return;
        }
        let reply = self
            .runtime
            .read(self.cancel.clone(), move |services| async move {
                let rows = services
                    .executor
                    .execute(
                        PICKER_QUERY.into(),
                        vec![
                            json!(format!("%{}%", escape_like(query.trim()))),
                            json!(viewer),
                            json!(serde_json::to_string(&recent).map_err(|error| {
                                ServiceError::Failed(error.to_string().into())
                            })?),
                        ],
                    )
                    .await
                    .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
                let more = rows.len() > 200;
                let rows = rows
                    .iter()
                    .take(200)
                    .map(|row| {
                        let id: Arc<str> = row["id"].as_str().unwrap_or_default().into();
                        PickerRow {
                            route: if row["kind"] == "shared" {
                                Route::SharedSession(id)
                            } else {
                                Route::Session(SessionId(id))
                            },
                            title: row["title"]
                                .as_str()
                                .filter(|title| !title.is_empty())
                                .unwrap_or("Untitled note")
                                .into(),
                            recent: row["rank"].as_u64().is_some_and(|rank| rank < 5),
                        }
                    })
                    .collect::<Vec<_>>();
                Ok((Arc::<[PickerRow]>::from(rows), more))
            });
        self.loading = true;
        self.message = "Searching titles…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    return;
                }
                this.loading = false;
                match result {
                    Ok((rows, more)) => {
                        this.rows = rows;
                        this.selected = 0;
                        this.scroll.scroll_to_item(0, gpui::ScrollStrategy::Top);
                        this.message = if more {
                            "Showing 200 results. Refine the title to find more."
                        } else if this.rows.is_empty() {
                            "No matching notes."
                        } else {
                            "↑ ↓ to select · Enter to open · Esc to close"
                        }
                        .into();
                    }
                    Err(error) => {
                        this.rows = Arc::from([]);
                        this.message = format!("Search failed: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn choose(&mut self, cx: &mut Context<Self>) {
        if !self.loading
            && let Some(row) = self.rows.get(self.selected)
        {
            cx.emit(PickerEvent::Open(row.route.clone()));
        }
    }

    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.input.read(cx).buffer.marked.is_some() {
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" => cx.emit(PickerEvent::Dismiss),
            "up" => self.selected = self.selected.saturating_sub(1),
            "down" => self.selected = (self.selected + 1).min(self.rows.len().saturating_sub(1)),
            _ => return,
        }
        self.scroll
            .scroll_to_item(self.selected, gpui::ScrollStrategy::Center);
        cx.stop_propagation();
        cx.notify();
    }
}

impl Drop for NotePicker {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Render for NotePicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        div()
            .id("open-note-picker")
            .w_full()
            .max_w(px(520.))
            .h(px(420.))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(colors.popover)
            .border_1()
            .border_color(colors.border)
            .rounded(px(8.))
            .shadow_lg()
            .on_key_down(cx.listener(Self::key))
            .child(self.input.clone())
            .child(
                div().flex_1().min_h_0().child(
                    uniform_list(
                        "picker-rows",
                        self.rows.len(),
                        cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|index| {
                                    let row = this.rows[index].clone();
                                    div()
                                        .id(index)
                                        .h(px(44.))
                                        .px_2()
                                        .rounded(px(8.))
                                        .bg(if index == this.selected {
                                            colors.accent
                                        } else {
                                            colors.popover
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.selected = index;
                                            this.choose(cx);
                                        }))
                                        .child(
                                            div()
                                                .text_sm()
                                                .truncate()
                                                .child(SharedString::from(row.title)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(colors.muted_foreground)
                                                .child(if row.recent {
                                                    "Recent"
                                                } else if matches!(
                                                    row.route,
                                                    Route::SharedSession(_)
                                                ) {
                                                    "Shared note"
                                                } else {
                                                    "Note"
                                                }),
                                        )
                                })
                                .collect()
                        }),
                    )
                    .track_scroll(&self.scroll)
                    .size_full(),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(self.message.clone()),
            )
    }
}
