use std::collections::HashMap;
use std::sync::Arc;

use anlg_listener_core::LiveTranscriptSegment;
use anlg_transcript::RenderedTranscriptSegment;
use gpui::{
    Context, Entity, EventEmitter, ListAlignment, ListState, Render, SharedString, Subscription,
    Window, div, list, prelude::*, px,
};

use crate::ui::theme::theme;

#[derive(Clone)]
pub enum TranscriptAction {
    Seek(i64),
    Edit(String),
    Speaker(Arc<RenderedTranscriptSegment>),
}

struct SegmentRow {
    segment: Arc<RenderedTranscriptSegment>,
    active: bool,
}

impl EventEmitter<TranscriptAction> for SegmentRow {}

impl Render for SegmentRow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let speaker = self.segment.clone();
        div()
            .id(SharedString::from(self.segment.id.clone()))
            .p_3()
            .border_b_1()
            .border_color(colors.border)
            .when(self.active, |view| view.bg(colors.accent))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(
                        div()
                            .id("speaker")
                            .cursor_pointer()
                            .child(self.segment.speaker_label.clone())
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(TranscriptAction::Speaker(speaker.clone()))
                            })),
                    )
                    .child(format!(
                        "{:02}:{:02}",
                        self.segment.start_ms / 60_000,
                        self.segment.start_ms / 1000 % 60
                    )),
            )
            .child(div().flex().flex_wrap().gap_1().children(
                self.segment.words.iter().enumerate().map(|(index, word)| {
                    let id = word.id.clone();
                    let time = word.start_ms;
                    div()
                        .id(index)
                        .cursor_pointer()
                        .rounded_sm()
                        .hover(|style| style.bg(colors.accent))
                        .when(!word.is_final, |view| {
                            view.text_color(colors.muted_foreground)
                        })
                        .child(word.text.clone())
                        .on_click(cx.listener(move |_, event: &gpui::ClickEvent, _, cx| {
                            if event.click_count() == 2 {
                                if let Some(id) = &id {
                                    cx.emit(TranscriptAction::Edit(id.clone()));
                                }
                            } else {
                                cx.emit(TranscriptAction::Seek(time));
                            }
                        }))
                }),
            ))
    }
}

struct RowEntry {
    entity: Entity<SegmentRow>,
    _subscription: Subscription,
}

pub struct TranscriptView {
    rows: Vec<RowEntry>,
    indices: HashMap<String, usize>,
    list: ListState,
    follow: bool,
    active: Option<usize>,
}

impl EventEmitter<TranscriptAction> for TranscriptView {}

impl TranscriptView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let list = ListState::new(0, ListAlignment::Top, px(300.));
        list.set_scroll_handler(cx.listener(|this, event: &gpui::ListScrollEvent, _, _| {
            this.follow = !event.is_scrolled || event.visible_range.end == event.count;
        }));
        Self {
            rows: Vec::new(),
            indices: HashMap::new(),
            list,
            follow: true,
            active: None,
        }
    }

    pub fn replace(
        &mut self,
        segments: Vec<Arc<RenderedTranscriptSegment>>,
        cx: &mut Context<Self>,
    ) {
        let top = self.list.logical_scroll_top();
        let mut previous: HashMap<String, RowEntry> = self
            .rows
            .drain(..)
            .map(|entry| (entry.entity.read(cx).segment.id.clone(), entry))
            .collect();
        self.indices.clear();
        for segment in segments {
            let id = segment.id.clone();
            let entry = match previous.remove(&id) {
                Some(entry) => {
                    entry.entity.update(cx, |row, cx| {
                        row.segment = segment;
                        cx.notify();
                    });
                    entry
                }
                None => self.entry(segment, cx),
            };
            self.indices.insert(id, self.rows.len());
            self.rows.push(entry);
        }
        self.list.reset(self.rows.len());
        self.list.scroll_to(top);
        self.active = None;
        cx.notify();
    }

    fn entry(&self, segment: Arc<RenderedTranscriptSegment>, cx: &mut Context<Self>) -> RowEntry {
        let entity = cx.new(|_| SegmentRow {
            segment,
            active: false,
        });
        let subscription = cx.subscribe(&entity, |_, _, event: &TranscriptAction, cx| {
            cx.emit(event.clone())
        });
        RowEntry {
            entity,
            _subscription: subscription,
        }
    }

    pub fn live(
        &mut self,
        id: String,
        update: Option<Arc<LiveTranscriptSegment>>,
        cx: &mut Context<Self>,
    ) {
        if let Some(segment) = update {
            let segment = Arc::new(RenderedTranscriptSegment {
                provisional_speaker: None,
                id: segment.id.clone(),
                key: segment.key.clone(),
                speaker_label: segment.key.speaker_human_id.clone().unwrap_or_else(|| {
                    segment
                        .key
                        .speaker_index
                        .map(|index| format!("Speaker {}", index + 1))
                        .unwrap_or_else(|| "Speaker".into())
                }),
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                text: segment.text.clone(),
                words: segment.words.clone(),
            });
            if let Some(&index) = self.indices.get(&id) {
                self.rows[index].entity.update(cx, |row, cx| {
                    row.segment = segment;
                    cx.notify();
                });
                self.list.splice(index..index + 1, 1);
            } else {
                let index = self.rows.partition_point(|entry| {
                    entry.entity.read(cx).segment.start_ms <= segment.start_ms
                });
                let entry = self.entry(segment, cx);
                self.rows.insert(index, entry);
                self.list.splice(index..index, 1);
                self.reindex(index, cx);
            }
        } else if let Some(index) = self.indices.remove(&id) {
            self.rows.remove(index);
            self.list.splice(index..index + 1, 0);
            self.reindex(index, cx);
        }
        if self.follow && !self.rows.is_empty() {
            self.list.scroll_to_reveal_item(self.rows.len() - 1);
        }
        cx.notify();
    }

    fn reindex(&mut self, start: usize, cx: &Context<Self>) {
        for (index, entry) in self.rows.iter().enumerate().skip(start) {
            self.indices
                .insert(entry.entity.read(cx).segment.id.clone(), index);
        }
    }

    pub fn playback_position(&mut self, milliseconds: i64, cx: &mut Context<Self>) {
        let end = self
            .rows
            .partition_point(|entry| entry.entity.read(cx).segment.start_ms <= milliseconds);
        let active = end
            .checked_sub(1)
            .filter(|&index| self.rows[index].entity.read(cx).segment.end_ms >= milliseconds);
        if active == self.active {
            return;
        }
        if let Some(index) = self.active.take()
            && let Some(entry) = self.rows.get(index)
        {
            entry.entity.update(cx, |row, cx| {
                row.active = false;
                cx.notify();
            });
        }
        if let Some(index) = active {
            self.rows[index].entity.update(cx, |row, cx| {
                row.active = true;
                cx.notify();
            });
        }
        self.active = active;
    }
}

impl Render for TranscriptView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.rows.is_empty() {
            return div().p_4().child("No transcript yet.").into_any_element();
        }
        let entity = cx.entity().downgrade();
        list(self.list.clone(), move |index, _, cx| {
            entity
                .update(cx, |this, _| {
                    this.rows[index].entity.clone().into_any_element()
                })
                .unwrap_or_else(|_| div().into_any_element())
        })
        .size_full()
        .into_any_element()
    }
}
