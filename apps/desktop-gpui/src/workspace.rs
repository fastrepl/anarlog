use std::sync::Arc;

use chrono::{DateTime, Local, Utc};
use gpui::{
    AnyElement, ClickEvent, Context, Div, HighlightStyle, ListAlignment, ListState, Render,
    SharedString, Stateful, StyledText, TextStyle, Window, div, list, prelude::*, px,
};

use crate::db::{NotePreview, Store};
use crate::document::{Block, Span};
use crate::theme::Theme;
use crate::timeline::{self, Bucket, Precision, Timeline};

const SIDEBAR_WIDTH: f32 = 280.0;

enum Sessions {
    Loading,
    Ready(Timeline),
    Failed(String),
}

/// One line of the sidebar list; buckets and their rows share one flat list
/// so the variable-height `list` element can virtualize them together.
#[derive(Clone)]
enum SidebarRow {
    Header { bucket: usize },
    Session { bucket: usize, item: usize },
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
    sessions: Sessions,
    rows: Vec<SidebarRow>,
    list_state: ListState,
    selected: Option<String>,
    note: Note,
}

impl Workspace {
    pub fn new(store: Arc<Store>, cx: &mut Context<Self>) -> Self {
        let font_family = crate::theme::ui_font_family(cx.text_system()).map(SharedString::from);
        let mut this = Self {
            store,
            theme: Theme::light(),
            font_family,
            sessions: Sessions::Loading,
            rows: Vec::new(),
            list_state: ListState::new(0, ListAlignment::Top, px(400.0)),
            selected: None,
            note: Note::Empty,
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
        let task = self.store.list_sessions();
        cx.spawn(async move |this, cx| {
            let result = match task.await {
                Ok(Ok(rows)) => Sessions::Ready(timeline::build(&rows, Utc::now(), &Local)),
                Ok(Err(error)) => Sessions::Failed(error.to_string()),
                Err(error) => Sessions::Failed(error.to_string()),
            };
            this.update(cx, |this, cx| {
                this.sessions = result;
                this.rebuild_rows();
                if this.selected.is_none()
                    && let Sessions::Ready(timeline) = &this.sessions
                    && let Some(first) = timeline.buckets.first().and_then(|b| b.items.first())
                {
                    let first_id = first.id.clone();
                    this.select(first_id, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        if let Sessions::Ready(timeline) = &self.sessions {
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

    fn render_sidebar(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let (count, has_more_future_items) = match &self.sessions {
            Sessions::Ready(timeline) => (
                timeline
                    .buckets
                    .iter()
                    .map(|b| b.items.len())
                    .sum::<usize>(),
                timeline.has_more_future_items,
            ),
            _ => (0, false),
        };
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("Notes"))
            .child(div().text_sm().text_color(theme.text_muted).child(
                if matches!(self.sessions, Sessions::Ready(_)) {
                    count.to_string()
                } else {
                    String::new()
                },
            ));

        let body = match &self.sessions {
            Sessions::Loading => self.render_message("Loading notes…"),
            Sessions::Failed(error) => self.render_error(error.clone()),
            Sessions::Ready(_) if self.rows.is_empty() => self.render_message("No notes yet."),
            Sessions::Ready(_) => div().flex_1().min_h_0().child(
                list(
                    self.list_state.clone(),
                    cx.processor(|this, index: usize, _window, cx| {
                        this.render_sidebar_row(index, cx)
                    }),
                )
                .size_full(),
            ),
        };

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(SIDEBAR_WIDTH))
            .flex_shrink_0()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border)
            .child(header)
            .child(body)
            .when(has_more_future_items, |sidebar| {
                sidebar.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .border_t_1()
                        .border_color(theme.border)
                        .child("More upcoming notes are in the calendar."),
                )
            })
    }

    fn render_sidebar_row(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let Sessions::Ready(timeline) = &self.sessions else {
            return div().into_any_element();
        };
        match self.rows.get(index) {
            Some(SidebarRow::Header { bucket }) => self
                .render_bucket_header(&timeline.buckets[*bucket])
                .into_any_element(),
            Some(SidebarRow::Session { bucket, item }) => {
                let bucket = &timeline.buckets[*bucket];
                self.render_session_row(index, &bucket.items[*item], bucket.precision, cx)
                    .into_any_element()
            }
            None => div().into_any_element(),
        }
    }

    fn render_bucket_header(&self, bucket: &Bucket) -> Div {
        div()
            .px_3()
            .pt_3()
            .pb_1()
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(self.theme.text_muted)
            .child(SharedString::from(bucket.label.clone()))
    }

    fn render_session_row(
        &self,
        index: usize,
        item: &timeline::Item,
        precision: Precision,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let session_id = item.id.clone();
        let selected = self.selected.as_deref() == Some(item.id.as_str());
        let title = display_title(&item.title);
        let time = SharedString::from(timeline::format_display_time(
            item.timestamp,
            precision,
            Utc::now(),
            &Local,
        ));

        div()
            .id(index)
            .flex()
            .flex_col()
            .gap_0p5()
            .mx_2()
            .my_0p5()
            .px_2()
            .py_1p5()
            .rounded_md()
            .cursor_pointer()
            .when(selected, |row| row.bg(theme.selected))
            .when(!selected, |row| row.hover(|style| style.bg(theme.hover)))
            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                this.select(session_id.clone(), cx);
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().text_sm().truncate().child(title))
                    .when(item.locked, |row| {
                        row.child(div().text_xs().text_color(theme.text_muted).child("locked"))
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .truncate()
                    .child(time),
            )
    }

    fn render_note(&self, window: &Window, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let content: AnyElement = match &self.note {
            Note::Empty => self
                .render_message("Select a note to read it.")
                .into_any_element(),
            Note::Loading => self.render_message("Loading…").into_any_element(),
            Note::Failed(error) => self.render_error(error.clone()).into_any_element(),
            Note::Ready { preview, tab } => {
                let blocks: &[Block] = match tab {
                    NoteTab::Memo => &preview.memo,
                    NoteTab::Enhanced(id) => preview
                        .enhanced
                        .iter()
                        .find(|doc| &doc.id == id)
                        .map(|doc| doc.blocks.as_slice())
                        .unwrap_or(&[]),
                };
                let has_content = blocks.iter().any(has_visible_content);
                let timestamp = timeline::session_timestamp(&preview.session, &Local);
                let renderer = DocumentRenderer::new(window, theme, self.font_family.clone());

                div()
                    .id("note")
                    .flex()
                    .flex_col()
                    .size_full()
                    .overflow_y_scroll()
                    .px_8()
                    .py_6()
                    .gap_4()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(display_title(&preview.session.title)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child(SharedString::from(display_date(timestamp))),
                    )
                    .child(self.render_tabs(preview, tab, cx))
                    .when(!has_content, |body| {
                        body.child(
                            div()
                                .text_color(theme.text_muted)
                                .child("This note is empty."),
                        )
                    })
                    .when(has_content, |body| {
                        body.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .max_w(px(720.0))
                                .line_height(px(24.0))
                                .children(renderer.blocks(blocks, 0)),
                        )
                    })
                    .into_any_element()
            }
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(theme.background)
            .child(content)
    }

    /// Enhanced notes first, then the memo, matching the app's tab strip.
    fn render_tabs(&self, preview: &NotePreview, current: &NoteTab, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let mut tabs: Vec<(NoteTab, SharedString)> = preview
            .enhanced
            .iter()
            .map(|doc| {
                let label = if doc.title.trim().is_empty() {
                    "Summary".to_string()
                } else {
                    doc.title.clone()
                };
                (NoteTab::Enhanced(doc.id.clone()), SharedString::from(label))
            })
            .collect();
        tabs.push((NoteTab::Memo, "Memo".into()));

        div()
            .flex()
            .gap_1()
            .children(tabs.into_iter().enumerate().map(|(index, (tab, label))| {
                let active = *current == tab;
                div()
                    .id(("tab", index))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .when(active, |t| t.bg(theme.selected))
                    .when(!active, |t| {
                        t.text_color(theme.text_muted)
                            .hover(|style| style.bg(theme.hover))
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.set_tab(tab.clone(), cx);
                    }))
                    .child(label)
            }))
    }

    fn render_status_bar(&self) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .items_center()
            .px_3()
            .h(px(24.0))
            .flex_shrink_0()
            .text_xs()
            .text_color(theme.text_muted)
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(
                div()
                    .truncate()
                    .child(format!("Reading {}", self.store.path().display())),
            )
    }

    fn render_message(&self, message: impl Into<SharedString>) -> Div {
        div()
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .p_4()
            .text_sm()
            .text_color(self.theme.text_muted)
            .child(message.into())
    }

    fn render_error(&self, message: String) -> Div {
        div()
            .flex()
            .flex_1()
            .items_start()
            .p_4()
            .text_sm()
            .text_color(self.theme.danger)
            .child(message)
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.text)
            .text_size(px(14.0))
            .when_some(self.font_family.clone(), |root, family| {
                root.font_family(family)
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child(self.render_note(window, cx)),
            )
            .child(self.render_status_bar())
    }
}

fn has_visible_content(block: &Block) -> bool {
    match block {
        Block::Paragraph(spans) => spans.iter().any(|span| !span.text.trim().is_empty()),
        Block::Heading { spans, .. } => spans.iter().any(|span| !span.text.trim().is_empty()),
        Block::List { items, .. } => items
            .iter()
            .any(|item| item.checked.is_some() || item.blocks.iter().any(has_visible_content)),
        Block::Blockquote(blocks) => blocks.iter().any(has_visible_content),
        Block::Code(code) => !code.trim().is_empty(),
        Block::HorizontalRule | Block::Image { .. } => true,
    }
}

/// Renders parsed documents the way the ProseMirror editor shows them:
/// block layout from the tree, inline marks as styled text runs.
struct DocumentRenderer {
    base: TextStyle,
    theme: Theme,
}

impl DocumentRenderer {
    fn new(window: &Window, theme: Theme, font_family: Option<SharedString>) -> Self {
        let mut base = window.text_style();
        base.font_size = px(14.0).into();
        base.color = theme.text.into();
        if let Some(family) = font_family {
            base.font_family = family;
        }
        Self { base, theme }
    }

    fn blocks(&self, blocks: &[Block], depth: usize) -> Vec<AnyElement> {
        blocks
            .iter()
            .map(|block| self.block(block, depth))
            .collect()
    }

    fn block(&self, block: &Block, depth: usize) -> AnyElement {
        let theme = self.theme;
        match block {
            Block::Paragraph(spans) => div()
                .min_h(px(24.0))
                .child(self.text(spans, &self.base))
                .into_any_element(),
            Block::Heading { level, spans } => {
                let mut style = self.base.clone();
                style.font_weight = gpui::FontWeight::SEMIBOLD;
                style.font_size = match level {
                    1 => px(24.0),
                    2 => px(20.0),
                    3 => px(17.0),
                    _ => px(15.0),
                }
                .into();
                div()
                    .mt_2()
                    .text_size(style.font_size.to_pixels(px(16.0)))
                    .line_height(px(32.0))
                    .child(self.text(spans, &style))
                    .into_any_element()
            }
            Block::List { ordered, items } => div()
                .flex()
                .flex_col()
                .gap_1()
                .children(items.iter().enumerate().map(|(index, item)| {
                    let marker = match (item.checked, ordered) {
                        (Some(true), _) => "☑".to_string(),
                        (Some(false), _) => "☐".to_string(),
                        (None, true) => format!("{}.", index + 1),
                        (None, false) => "•".to_string(),
                    };
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_shrink_0()
                                .w(px(20.0))
                                .text_color(theme.text_muted)
                                .child(SharedString::from(marker)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .min_w_0()
                                .children(self.blocks(&item.blocks, depth + 1)),
                        )
                }))
                .into_any_element(),
            Block::Blockquote(blocks) => div()
                .flex()
                .flex_col()
                .gap_2()
                .pl_3()
                .border_l_2()
                .border_color(theme.border)
                .text_color(theme.text_muted)
                .children(self.blocks(blocks, depth + 1))
                .into_any_element(),
            Block::Code(code) => div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(theme.sidebar)
                .font_family("monospace")
                .text_sm()
                .child(SharedString::from(code.clone()))
                .into_any_element(),
            Block::HorizontalRule => div().my_2().h(px(1.0)).bg(theme.border).into_any_element(),
            Block::Image { alt } => div()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_dashed()
                .border_color(theme.border)
                .text_color(theme.text_muted)
                .text_sm()
                .child(SharedString::from(if alt.is_empty() {
                    "Image".to_string()
                } else {
                    format!("Image: {alt}")
                }))
                .into_any_element(),
        }
    }

    fn text(&self, spans: &[Span], base: &TextStyle) -> StyledText {
        let mut text = String::new();
        let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
        for span in spans {
            let start = text.len();
            text.push_str(&span.text);
            let highlight = HighlightStyle {
                font_weight: span.bold.then_some(gpui::FontWeight::BOLD),
                font_style: span.italic.then_some(gpui::FontStyle::Italic),
                color: span.link.is_some().then(|| self.theme.link.into()),
                background_color: span.code.then(|| self.theme.selected.into()),
                underline: (span.underline || span.link.is_some()).then(|| gpui::UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(self.theme.link.into()),
                    wavy: false,
                }),
                strikethrough: span.strike.then(|| gpui::StrikethroughStyle {
                    thickness: px(1.0),
                    color: Some(self.theme.text_muted.into()),
                }),
                ..HighlightStyle::default()
            };
            if highlight != HighlightStyle::default() {
                highlights.push((start..text.len(), highlight));
            }
        }
        StyledText::new(text).with_default_highlights(base, highlights)
    }
}

fn display_title(title: &str) -> SharedString {
    let title = title.trim();
    if title.is_empty() {
        "Untitled".into()
    } else {
        title.to_string().into()
    }
}

fn display_date(timestamp: Option<DateTime<Utc>>) -> String {
    timestamp
        .map(|ts| timeline::format_display_time(ts, Precision::Date, Utc::now(), &Local))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_title_falls_back_to_untitled() {
        assert_eq!(display_title("  "), SharedString::from("Untitled"));
        assert_eq!(display_title(" Standup "), SharedString::from("Standup"));
    }
}
