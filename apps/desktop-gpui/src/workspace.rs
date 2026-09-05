use std::ops::Range;
use std::sync::Arc;

use anlg_db_app::SessionListItem;
use gpui::{
    AnyElement, ClickEvent, Context, Div, Render, SharedString, Stateful, Window, div, prelude::*,
    px, uniform_list,
};

use crate::db::{NotePreview, Store};
use crate::theme::Theme;

const SIDEBAR_WIDTH: f32 = 280.0;

enum Sessions {
    Loading,
    Ready(Vec<SessionListItem>),
    Failed(String),
}

enum Note {
    Empty,
    Loading,
    Ready(NotePreview),
    Failed(String),
}

pub struct Workspace {
    store: Arc<Store>,
    theme: Theme,
    sessions: Sessions,
    selected: Option<String>,
    note: Note,
}

impl Workspace {
    pub fn new(store: Arc<Store>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            store,
            theme: Theme::light(),
            sessions: Sessions::Loading,
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
                Ok(Ok(sessions)) => Sessions::Ready(sessions),
                Ok(Err(error)) => Sessions::Failed(error.to_string()),
                Err(error) => Sessions::Failed(error.to_string()),
            };
            this.update(cx, |this, cx| {
                if let Sessions::Ready(sessions) = &result
                    && this.selected.is_none()
                    && let Some(first) = sessions.first()
                {
                    let first_id = first.id.clone();
                    this.select(first_id, cx);
                }
                this.sessions = result;
                cx.notify();
            })
            .ok();
        })
        .detach();
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
                Ok(Ok(Some(note))) => Note::Ready(note),
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

    fn render_sidebar(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("Notes"))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .child(match &self.sessions {
                        Sessions::Ready(sessions) => sessions.len().to_string(),
                        _ => String::new(),
                    }),
            );

        let body = match &self.sessions {
            Sessions::Loading => self.render_message("Loading notes…"),
            Sessions::Failed(error) => self.render_error(error.clone()),
            Sessions::Ready(sessions) if sessions.is_empty() => {
                self.render_message("No notes yet.")
            }
            Sessions::Ready(sessions) => {
                let count = sessions.len();
                div().flex_1().min_h_0().child(
                    uniform_list(
                        "sessions",
                        count,
                        cx.processor(|this, range: Range<usize>, _window, cx| {
                            let Sessions::Ready(sessions) = &this.sessions else {
                                return Vec::new();
                            };
                            range
                                .filter_map(|index| sessions.get(index).map(|s| (index, s)))
                                .map(|(index, session)| this.render_session_row(index, session, cx))
                                .collect()
                        }),
                    )
                    .h_full(),
                )
            }
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
    }

    fn render_session_row(
        &self,
        index: usize,
        session: &SessionListItem,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let session_id = session.id.clone();
        let selected = self.selected.as_deref() == Some(session.id.as_str());
        let title = display_title(&session.title);
        let subtitle = display_date(&session.started_at, &session.created_at);

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
            .child(div().text_sm().truncate().child(title))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .truncate()
                    .child(subtitle),
            )
    }

    fn render_note(&self) -> Div {
        let theme = self.theme;
        let content: AnyElement = match &self.note {
            Note::Empty => self
                .render_message("Select a note to read it.")
                .into_any_element(),
            Note::Loading => self.render_message("Loading…").into_any_element(),
            Note::Failed(error) => self.render_error(error.clone()).into_any_element(),
            Note::Ready(note) => {
                let blocks = markdown_blocks(&note.body);

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
                            .child(display_title(&note.title)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child(display_date(&note.started_at, "")),
                    )
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
                    .child(self.render_note()),
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

/// Sessions store RFC 3339 timestamps; show the calendar date and time without
/// pulling in a date-time crate until locale-aware formatting is needed.
fn display_date(started_at: &str, fallback: &str) -> SharedString {
    let source = if started_at.trim().is_empty() {
        fallback
    } else {
        started_at
    };
    let (date, rest) = source.split_once('T').unwrap_or((source, ""));
    let time = rest.get(..5).unwrap_or("");
    if time.is_empty() {
        date.to_string().into()
    } else {
        format!("{date} {time}").into()
    }
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

    #[test]
    fn display_date_prefers_started_at_and_trims_seconds() {
        assert_eq!(
            display_date("2026-09-01T09:05:33.120Z", "2026-08-31T00:00:00Z"),
            SharedString::from("2026-09-01 09:05")
        );
        assert_eq!(
            display_date("", "2026-08-31T23:59:00Z"),
            SharedString::from("2026-08-31 23:59")
        );
        assert_eq!(
            display_date("2026-09-01", ""),
            SharedString::from("2026-09-01")
        );
    }
}
