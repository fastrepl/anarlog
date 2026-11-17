//! Shipping's memo extras below the editor (`note-input/meeting-screen-gallery.tsx` and
//! `note-input/meeting-chat-highlights.tsx`): the shared screens and meeting chat captured
//! while the meeting was recorded. Nothing renders until something was captured.

use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    path::PathBuf,
    sync::Arc,
};

use chrono::{DateTime, Local};
use desktop_runtime::{CancellationToken, SessionId};
use gpui::{
    Context, FontWeight, HighlightStyle, InteractiveText, Render, SharedString, StyledText, Task,
    UnderlineStyle, Window, div, prelude::*, px,
};
use serde_json::{Value, json};

use crate::{
    editor::AttachmentService,
    meeting::schedule::{
        apps::MeetingChatDirection,
        records::{self, ChatRecord, ScreenRecord},
    },
    ui::theme::{MEDIUM, Theme, theme},
};

/// Both kinds in one watch: screens in capture order, then the latest chat.
const QUERY: &str = "SELECT 'meeting_screen' AS kind, id, body, created_at FROM (
        SELECT id, body, created_at FROM session_documents
        WHERE session_id = ?1 AND kind = 'meeting_screen' AND deleted_at IS NULL
        ORDER BY sort_order ASC, created_at ASC)
    UNION ALL
    SELECT 'meeting_chat' AS kind, id, body, created_at FROM (
        SELECT id, body, created_at, sort_order FROM (
            SELECT id, body, created_at, sort_order FROM session_documents
            WHERE session_id = ?1 AND kind = 'meeting_chat' AND deleted_at IS NULL
              AND length(CAST(body AS BLOB)) <= 16384
            ORDER BY sort_order DESC, created_at DESC, id DESC LIMIT 1000)
        ORDER BY sort_order, created_at, id)";

pub struct MeetingRecords {
    chat: Vec<ChatRecord>,
    screens: Vec<ScreenRecord>,
    sources: HashMap<String, Arc<PathBuf>>,
    resolving: HashSet<String>,
    attachments: AttachmentService,
    session: SessionId,
    _watch: Task<()>,
}

impl MeetingRecords {
    pub fn new(
        runtime: desktop_runtime::RuntimeHandle,
        attachments: AttachmentService,
        session: SessionId,
        cx: &mut Context<Self>,
    ) -> Self {
        let reply = runtime.watch_query(QUERY.into(), vec![json!(session)]);
        let watch = cx.spawn(async move |this, cx| {
            let Ok(mut watch) = async { reply?.receive().await }.await else {
                return;
            };
            loop {
                if watch.terminal_error().is_some() || watch.errors.try_recv().is_ok() {
                    break;
                }
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                if this.update(cx, |this, cx| this.replace(&rows, cx)).is_err()
                    || watch.snapshots.changed().await.is_err()
                {
                    break;
                }
            }
            let _ = watch.unsubscribe().await;
        });
        Self {
            chat: Vec::new(),
            screens: Vec::new(),
            sources: HashMap::new(),
            resolving: HashSet::new(),
            attachments,
            session,
            _watch: watch,
        }
    }

    fn replace(&mut self, rows: &[Value], cx: &mut Context<Self>) {
        let (screens, chat): (Vec<Value>, Vec<Value>) = rows
            .iter()
            .cloned()
            .partition(|row| row["kind"] == "meeting_screen");
        let chat = records::parse_chat(&chat);
        let screens = records::parse_screens(&screens);
        for record in &screens {
            self.resolve(record.attachment_id.clone(), cx);
        }
        if chat != self.chat || screens != self.screens {
            self.chat = chat;
            self.screens = screens;
            cx.notify();
        }
    }

    fn resolve(&mut self, id: String, cx: &mut Context<Self>) {
        if self.sources.contains_key(&id) || !self.resolving.insert(id.clone()) {
            return;
        }
        let attachments = self.attachments.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = attachments
                .resolve(session, id.clone(), CancellationToken::new())
                .await;
            let _ = this.update(cx, |this, cx| {
                this.resolving.remove(&id);
                if let Ok(preview) = result {
                    this.sources.insert(id, preview.path);
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

/// Shipping's section: `mx-auto mt-4 mb-6 w-full max-w-3xl rounded-xl border px-3 py-2.5`.
fn section(colors: Theme, title: &'static str) -> gpui::Div {
    div()
        .mx_auto()
        .mt_4()
        .mb_6()
        .w_full()
        .max_w(px(768.))
        .border_1()
        .border_color(colors.border.opacity(0.7))
        .bg(colors.muted.opacity(0.3))
        .px_3()
        .py(px(10.))
        .child(
            div()
                .mb_2()
                .text_xs()
                .line_height(px(16.))
                .font_weight(MEDIUM)
                .text_color(colors.muted_foreground)
                .child(title),
        )
}

fn local_time(captured_at: &str) -> String {
    DateTime::parse_from_rfc3339(captured_at)
        .map(|time| time.with_timezone(&Local).format("%-I:%M %p").to_string())
        .unwrap_or_default()
}

/// Shipping's `splitMeetingChatText`: the record's http(s) links found in the text.
fn link_ranges(text: &str, links: &[String]) -> Vec<(Range<usize>, String)> {
    let mut unique: Vec<&String> = Vec::new();
    for link in links {
        if text.contains(link.as_str()) && !unique.contains(&link) {
            unique.push(link);
        }
    }
    let mut ranges = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let next = unique
            .iter()
            .filter_map(|link| {
                text[cursor..]
                    .find(link.as_str())
                    .map(|at| (cursor + at, *link))
            })
            .min_by_key(|(at, _)| *at);
        let Some((at, link)) = next else {
            break;
        };
        ranges.push((at..at + link.len(), link.clone()));
        cursor = at + link.len();
    }
    ranges
}

fn chat_row(index: usize, record: &ChatRecord, colors: Theme) -> gpui::Div {
    let direction = match record.direction {
        Some(MeetingChatDirection::Outgoing) => Some("sent"),
        Some(MeetingChatDirection::Incoming) => Some("received"),
        None => None,
    };
    let metadata: Vec<&str> = [
        record.timestamp.as_deref(),
        record.sender.as_deref(),
        direction,
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.is_empty())
    .collect();
    let mut label = records::platform_label(&record.platform).to_owned();
    if !metadata.is_empty() {
        label = format!("{label} · {}", metadata.join(" · "));
    }
    let links = link_ranges(&record.text, &record.links);
    let highlight = HighlightStyle {
        color: Some(colors.primary),
        underline: Some(UnderlineStyle {
            thickness: px(1.),
            color: Some(colors.primary),
            wavy: false,
        }),
        ..HighlightStyle::default()
    };
    let styled = StyledText::new(SharedString::from(record.text.clone()))
        .with_highlights(links.iter().map(|(range, _)| (range.clone(), highlight)));
    let text = if links.is_empty() {
        styled.into_any_element()
    } else {
        let ranges = links.iter().map(|(range, _)| range.clone()).collect();
        InteractiveText::new(("meeting-chat-text", index), styled)
            .on_click(ranges, move |index, _, cx| cx.open_url(&links[index].1))
            .into_any_element()
    };
    div()
        .text_sm()
        .line_height(px(20.))
        .text_color(colors.foreground)
        .child(
            div()
                .text_xs()
                .line_height(px(16.))
                .text_color(colors.muted_foreground)
                .child(label),
        )
        .child(div().child(text))
}

impl Render for MeetingRecords {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let gallery = (!self.screens.is_empty()).then(|| {
            section(colors, "Shared screens").child(div().grid().grid_cols(2).gap_2().children(
                self.screens.iter().map(|record| {
                    let time = local_time(&record.captured_at);
                    let label = match record
                        .platform
                        .as_deref()
                        .and_then(records::platform_from_key)
                    {
                        Some(platform) => {
                            format!("{} · {time}", records::platform_label(&platform))
                        }
                        None => time,
                    };
                    let ratio = if record.width > 0 && record.height > 0 {
                        record.width as f32 / record.height as f32
                    } else {
                        16. / 9.
                    };
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .w_full()
                                .aspect_ratio(ratio)
                                .overflow_hidden()
                                .bg(colors.muted)
                                .children(self.sources.get(&record.attachment_id).map(|path| {
                                    gpui::img(path.as_ref().clone())
                                        .size_full()
                                        .object_fit(gpui::ObjectFit::Contain)
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .line_height(px(16.))
                                .text_color(colors.muted_foreground)
                                .child(label),
                        )
                }),
            ))
        });
        let chat = (!self.chat.is_empty()).then(|| {
            section(colors, "Meeting chat").child(
                div().flex().flex_col().gap_2().children(
                    self.chat
                        .iter()
                        .enumerate()
                        .map(|(index, record)| chat_row(index, record, colors)),
                ),
            )
        });
        let empty = gallery.is_none() && chat.is_none();
        // The memo editor's 24px bottom padding comes before shipping's sections.
        div()
            .w_full()
            .px_3()
            .when(!empty, |view| view.pt(px(24.)))
            .font_weight(FontWeight::NORMAL)
            .children(gallery)
            .children(chat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_links_the_record_lists_become_clickable_ranges() {
        let text = "Deck https://a.test/x and https://a.test/x again, not https://b.test";
        let ranges = link_ranges(text, &["https://a.test/x".into(), "https://c.test".into()]);
        assert_eq!(
            ranges
                .iter()
                .map(|(range, _)| &text[range.clone()])
                .collect::<Vec<_>>(),
            vec!["https://a.test/x", "https://a.test/x"]
        );
    }
}
