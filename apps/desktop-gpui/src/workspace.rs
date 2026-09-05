use std::sync::Arc;

use chrono::{DateTime, Local, Utc};
use gpui::{
    AnyElement, ClickEvent, Context, Div, ListAlignment, ListState, Render, SharedString, Stateful,
    Window, div, list, prelude::*, px,
};

use crate::db::{NotePreview, Store};
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
    sessions: Sessions,
    rows: Vec<SidebarRow>,
    list_state: ListState,
    selected: Option<String>,
    note: Note,
}

impl Workspace {
    pub fn new(store: Arc<Store>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            store,
            theme: Theme::light(),
            sessions: Sessions::Loading,
            rows: Vec::new(),
            list_state: ListState::new(0, ListAlignment::Top, px(400.0)),
            selected: None,
            note: Note::Empty,
        };
        this.reload_sessions(cx);
        this
    }

    fn reload_sessions(&mut self, cx: &mut Context<Self>) {
        self.sessions = Sessions::Loading;
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

        let task = self.store.load_note(session_id.clone());
        cx.spawn(async move |this, cx| {
            let result = match task.await {
                Ok(Ok(Some(preview))) => {
                    let tab = match preview.enhanced.first() {
                        Some(first) => NoteTab::Enhanced(first.id.clone()),
                        None => NoteTab::Memo,
                    };
                    Note::Ready { preview, tab }
                }
                Ok(Ok(None)) => Note::Failed("This note no longer exists.".to_string()),
                Ok(Err(error)) => Note::Failed(error.to_string()),
                Err(error) => Note::Failed(error.to_string()),
            };
            this.update(cx, |this, cx| {
                // A newer selection may have raced this load; keep the latest.
                if this.selected.as_deref() == Some(session_id.as_str()) {
                    this.note = result;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
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

    fn render_note(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let content: AnyElement = match &self.note {
            Note::Empty => self
                .render_message("Select a note to read it.")
                .into_any_element(),
            Note::Loading => self.render_message("Loading…").into_any_element(),
            Note::Failed(error) => self.render_error(error.clone()).into_any_element(),
            Note::Ready { preview, tab } => {
                let body = match tab {
                    NoteTab::Memo => preview.memo.as_str(),
                    NoteTab::Enhanced(id) => preview
                        .enhanced
                        .iter()
                        .find(|doc| &doc.id == id)
                        .map(|doc| doc.body.as_str())
                        .unwrap_or(""),
                };
                let blocks = markdown_blocks(body);
                let timestamp = timeline::session_timestamp(&preview.session, &Local);

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
                    .when(blocks.is_empty(), |body| {
                        body.child(
                            div()
                                .text_color(theme.text_muted)
                                .child("This note is empty."),
                        )
                    })
                    .children(blocks.into_iter().map(|block| render_block(block, theme)))
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.text)
            .text_size(px(14.0))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child(self.render_note(cx)),
            )
            .child(self.render_status_bar())
    }
}

/// Coarse block structure of the Markdown produced by `anlg_tiptap`, enough to
/// read a note. Inline marks (bold, links) stay literal until the editor lands.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Block {
    Heading { level: usize, text: String },
    Bullet(String),
    Paragraph(String),
}

fn markdown_blocks(body: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |paragraph: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if !paragraph.is_empty() {
            blocks.push(Block::Paragraph(paragraph.join(" ")));
            paragraph.clear();
        }
    };

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush(&mut paragraph, &mut blocks);
            continue;
        }

        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block::Heading {
                level: hashes,
                text: trimmed[hashes..].trim().to_string(),
            });
            continue;
        }

        let bullet = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| {
                let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
                (digits > 0)
                    .then(|| trimmed[digits..].strip_prefix(". "))
                    .flatten()
            });
        if let Some(item) = bullet {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block::Bullet(item.trim().to_string()));
            continue;
        }

        paragraph.push(trimmed);
    }
    flush(&mut paragraph, &mut blocks);
    blocks
}

fn render_block(block: Block, theme: Theme) -> Div {
    let base = div().max_w(px(720.0)).line_height(px(24.0));
    match block {
        Block::Heading { level, text } => base
            .mt_2()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .map(|heading| match level {
                1 => heading.text_xl(),
                2 => heading.text_lg(),
                _ => heading,
            })
            .child(SharedString::from(text)),
        Block::Bullet(text) => base
            .flex()
            .gap_2()
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(theme.text_muted)
                    .child("•"),
            )
            .child(div().child(SharedString::from(text))),
        Block::Paragraph(text) => base.child(SharedString::from(text)),
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
    fn markdown_blocks_split_headings_bullets_and_paragraphs() {
        let body = "## Agenda\n\n- First item\n* Second item\n1. Third item\n\nA paragraph\nthat wraps.\n\n#notatag stays text\n";
        assert_eq!(
            markdown_blocks(body),
            vec![
                Block::Heading {
                    level: 2,
                    text: "Agenda".to_string()
                },
                Block::Bullet("First item".to_string()),
                Block::Bullet("Second item".to_string()),
                Block::Bullet("Third item".to_string()),
                Block::Paragraph("A paragraph that wraps.".to_string()),
                Block::Paragraph("#notatag stays text".to_string()),
            ]
        );
        assert!(markdown_blocks("  \n\n").is_empty());
    }

    #[test]
    fn display_title_falls_back_to_untitled() {
        assert_eq!(display_title("  "), SharedString::from("Untitled"));
        assert_eq!(display_title(" Standup "), SharedString::from("Standup"));
    }
}
