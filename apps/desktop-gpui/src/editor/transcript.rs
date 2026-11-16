//! The open note's Transcript view: shipping's `TranscriptContent` screens (listening,
//! finalizing, batch, empty) and its segment list (speaker label in its segment color, then the
//! words), rendered by the same `anlg_transcript` segmenter. While a meeting is live, stored
//! segments merge with the live ones and partial words show faded and italic.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use anlg_listener_core::LiveTranscriptSegment;
use anlg_transcript::{
    ChannelProfile, RenderTranscriptRequest, RenderedTranscriptSegment, SegmentKey, SegmentWord,
    SpeakerLabeler, render_transcript_segments,
};
use desktop_runtime::{RuntimeHandle, SessionId};
use gpui::{
    App, Context, Entity, FontStyle, FontWeight, HighlightStyle, Hsla, Render, ScrollHandle,
    SharedString, StyledText, Subscription, Task, Window, div, prelude::*, px, rgb, svg,
};
use serde_json::{Value, json};

use crate::{
    meeting::{
        live::{CaptureMode, LiveMeeting, LiveState, SessionMode},
        model::Transcript,
        widgets::{dancing_sticks, spinner},
    },
    ui::theme::{RADIUS, Theme, theme},
};

pub struct TranscriptView {
    session: SessionId,
    segments: Vec<RenderedTranscriptSegment>,
    loaded: bool,
    audio: bool,
    error: Option<String>,
    live: Option<Entity<LiveState>>,
    scroll: ScrollHandle,
    _watch: Task<()>,
    _live: Option<Subscription>,
}

impl TranscriptView {
    pub fn new(runtime: RuntimeHandle, session: SessionId, cx: &mut Context<Self>) -> Self {
        let reply = runtime.watch_query(
            "SELECT id, started_at_ms, words_json, speaker_hints_json FROM transcripts
             WHERE session_id = ? AND deleted_at IS NULL ORDER BY started_at_ms, id"
                .into(),
            vec![json!(session)],
        );
        let watch = cx.spawn(async move |this, cx| {
            let mut watch = match async { reply?.receive().await }.await {
                Ok(watch) => watch,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            loop {
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                let rendered = cx
                    .background_executor()
                    .spawn(async move { segments(&rows) })
                    .await;
                let error = watch.terminal_error().map(|error| error.to_string());
                let terminal = error.is_some();
                if this
                    .update(cx, |this, cx| {
                        match rendered {
                            Ok(segments) => this.segments = segments,
                            Err(error) => this.error = Some(error),
                        }
                        if let Some(error) = error {
                            this.error = Some(error);
                        }
                        this.loaded = true;
                        this.follow(cx);
                        cx.notify();
                    })
                    .is_err()
                    || terminal
                    || watch.snapshots.changed().await.is_err()
                {
                    break;
                }
            }
            let _ = watch.unsubscribe().await;
        });
        let live = LiveState::global(cx);
        let observe = live.as_ref().map(|live| {
            cx.observe(live, |this, _, cx| {
                this.follow(cx);
                cx.notify();
            })
        });
        Self {
            session,
            segments: Vec::new(),
            loaded: false,
            audio: false,
            error: None,
            live,
            scroll: ScrollHandle::new(),
            _watch: watch,
            _live: observe,
        }
    }

    /// Whether the session has saved audio, which decides the empty screen's copy.
    pub fn set_audio(&mut self, audio: bool, cx: &mut Context<Self>) {
        if self.audio != audio {
            self.audio = audio;
            cx.notify();
        }
    }

    fn live(&self, cx: &App) -> Option<LiveMeeting> {
        self.live
            .as_ref()
            .and_then(|live| live.read(cx).session(&self.session).cloned())
            .filter(LiveMeeting::live)
    }

    /// Shipping keeps a live transcript pinned to its end while the reader is near the bottom.
    fn follow(&mut self, cx: &App) {
        if self.live(cx).is_some() {
            let offset = self.scroll.offset().y;
            let max = self.scroll.max_offset().y;
            if max + offset < px(120.) {
                self.scroll.scroll_to_bottom();
            }
        }
    }
}

/// Shipping's `useTranscriptScreen`.
#[derive(Debug, PartialEq)]
enum Screen {
    Listening { finalizing: bool },
    Batch { requested_live: bool },
    Empty { audio: bool },
    Ready,
}

fn screen(live: Option<&LiveMeeting>, has_words: bool, audio: bool) -> Screen {
    let visible = has_words || live.is_some_and(|live| !live.segments.is_empty());
    let active = live.is_some_and(|live| live.mode == SessionMode::Active);
    if active && live.is_some_and(|live| live.capture == CaptureMode::RecordOnly) {
        return Screen::Batch {
            requested_live: false,
        };
    }
    if !visible && live.is_some_and(|live| live.paused.is_some()) {
        return Screen::Batch {
            requested_live: true,
        };
    }
    if !visible && let Some(live) = live {
        return Screen::Listening {
            finalizing: live.mode == SessionMode::Finalizing,
        };
    }
    if !visible {
        return Screen::Empty { audio };
    }
    Screen::Ready
}

fn segments(rows: &[Value]) -> Result<Vec<RenderedTranscriptSegment>, String> {
    let parse = |row: &Value, key: &str| -> Result<Vec<Value>, String> {
        serde_json::from_str(row[key].as_str().unwrap_or("[]")).map_err(|error| error.to_string())
    };
    let mut transcripts = Vec::new();
    for row in rows {
        let words = parse(row, "words_json")?
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let transcript = Transcript {
            id: row["id"].as_str().unwrap_or_default().into(),
            started_at: row["started_at_ms"].as_i64().unwrap_or(0),
            revision: 0,
            sequence: 0,
            words,
            hints: parse(row, "speaker_hints_json")?,
        };
        transcripts.push(
            transcript
                .render_input()
                .map_err(|error| error.to_string())?,
        );
    }
    Ok(render_transcript_segments(RenderTranscriptRequest {
        speaker_context: None,
        preview: None,
        transcripts,
        participant_human_ids: Vec::new(),
        self_human_id: None,
        humans: Vec::new(),
    }))
}

/// One rendered segment: who spoke, and their words.
#[derive(Debug)]
struct Row {
    key: SegmentKey,
    label: SharedString,
    start_ms: i64,
    words: Vec<SegmentWord>,
    text: String,
}

/// Shipping's `mergeRenderedAndLiveSegments` then `mergeAdjacentSpeakerSegments`: stored words
/// the live stream still carries come from the live side, and consecutive segments from one
/// speaker join.
fn merge(stored: &[RenderedTranscriptSegment], live: &[Arc<LiveTranscriptSegment>]) -> Vec<Row> {
    let live_ids: HashSet<&str> = live.iter().map(|segment| segment.id.as_str()).collect();
    let live_words: HashSet<&str> = live
        .iter()
        .flat_map(|segment| segment.words.iter().filter_map(|word| word.id.as_deref()))
        .collect();
    let mut labeler = SpeakerLabeler::new();
    let mut rows: Vec<Row> = Vec::new();
    for segment in stored {
        if segment.key.speaker_human_id.is_none() {
            labeler.unknown_speaker_number(&segment.key);
        }
        if live_ids.contains(segment.id.as_str()) {
            continue;
        }
        let words: Vec<SegmentWord> = segment
            .words
            .iter()
            .filter(|word| !word.id.as_deref().is_some_and(|id| live_words.contains(id)))
            .cloned()
            .collect();
        if words.is_empty() && !segment.words.is_empty() {
            continue;
        }
        rows.push(Row {
            key: segment.key.clone(),
            label: segment.speaker_label.clone().into(),
            start_ms: segment.start_ms,
            words,
            text: segment.text.clone(),
        });
    }
    for segment in live {
        let label = rows
            .iter()
            .find(|row| row.key == segment.key)
            .map(|row| row.label.clone())
            .unwrap_or_else(|| {
                format!("Speaker {}", labeler.unknown_speaker_number(&segment.key)).into()
            });
        rows.push(Row {
            key: segment.key.clone(),
            label,
            start_ms: segment.start_ms,
            words: segment.words.clone(),
            text: segment.text.clone(),
        });
    }
    if live.is_empty() {
        return rows;
    }
    rows.sort_by_key(|row| row.start_ms);
    let mut merged: Vec<Row> = Vec::new();
    for row in rows {
        match merged.last_mut() {
            Some(last) if last.key == row.key => {
                last.words.extend(row.words);
                last.text.push(' ');
                last.text.push_str(&row.text);
            }
            _ => merged.push(row),
        }
    }
    merged
}

/// Words trimmed and joined by spaces, with partial words styled `opacity-60 italic`.
fn row_text(row: &Row) -> (String, Vec<(Range<usize>, HighlightStyle)>) {
    if row.words.is_empty() {
        return (
            row.text.split_whitespace().collect::<Vec<_>>().join(" "),
            Vec::new(),
        );
    }
    let partial = HighlightStyle {
        font_style: Some(FontStyle::Italic),
        fade_out: Some(0.4),
        ..Default::default()
    };
    let mut text = String::new();
    let mut highlights = Vec::new();
    for word in &row.words {
        if !text.is_empty() {
            text.push(' ');
        }
        let start = text.len();
        text.push_str(word.text.trim());
        if !word.is_final {
            highlights.push((start..text.len(), partial));
        }
    }
    (text, highlights)
}

/// Shipping's `getSegmentColor`: golden-angle hues in OKLCH, remote speakers rotated 180°.
fn segment_color(key: &SegmentKey, dark: bool) -> Hsla {
    let mut index = key.speaker_index.unwrap_or(0) as u32 as f64;
    if let Some(human) = &key.speaker_human_id {
        let mut hash = 0u32;
        for unit in human.encode_utf16() {
            hash = hash.wrapping_mul(31).wrapping_add(u32::from(unit));
        }
        index = f64::from(hash);
    }
    let offset = if key.speaker_human_id.is_none() && key.channel == ChannelProfile::RemoteParty {
        180.
    } else {
        0.
    };
    let hue = (10. + index * 137.508 + offset) % 360.;
    oklch(if dark { 0.72 } else { 0.55 }, 0.15, hue)
}

fn oklch(lightness: f64, chroma: f64, hue: f64) -> Hsla {
    let (a, b) = (
        chroma * hue.to_radians().cos(),
        chroma * hue.to_radians().sin(),
    );
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    let channel = |linear: f64| {
        let linear = linear.clamp(0., 1.);
        let value = if linear <= 0.003_130_8 {
            12.92 * linear
        } else {
            1.055 * linear.powf(1. / 2.4) - 0.055
        };
        (value * 255.).round() as u32
    };
    let red = channel(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s);
    let green = channel(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s);
    let blue = channel(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s);
    rgb((red << 16) | (green << 8) | blue).into()
}

/// Shipping's centered transcript status: icon, `text-base` title and muted description.
fn status(
    colors: Theme,
    icon: gpui::AnyElement,
    title: &'static str,
    description: &'static str,
) -> gpui::Div {
    // Shipping's transcript container adds `pt-2` above every screen.
    div()
        .size_full()
        .min_h(px(400.))
        .pt_2()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .px_6()
        .text_center()
        .child(div().mb_5().child(icon))
        .child(
            div()
                .max_w(px(448.))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_base()
                        .line_height(px(24.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_sm()
                        .line_height(px(22.75))
                        .text_color(colors.muted_foreground)
                        .child(description),
                ),
        )
}

fn waveform(colors: Theme) -> gpui::AnyElement {
    svg()
        .path("meeting/AudioWaveformIcon.svg")
        .size(px(36.))
        .text_color(colors.muted_foreground)
        .into_any_element()
}

/// Shipping's `LiveTranscriptInterruptedNotice`.
fn paused_notice(colors: Theme, message: SharedString) -> impl IntoElement {
    div().w_full().flex().justify_center().child(
        div()
            .max_w(px(448.))
            .px_6()
            .text_center()
            .text_sm()
            .line_height(px(22.75))
            .text_color(colors.muted_foreground)
            .child(message),
    )
}

/// A small shadcn `Button`, shown disabled where the native app has no implementation yet.
fn disabled_button(colors: Theme, label: &'static str, primary: bool) -> impl IntoElement {
    div()
        .h_7()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .rounded(px(RADIUS))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .opacity(0.5)
        .map(|button| {
            if primary {
                button
                    .bg(colors.primary)
                    .text_color(colors.primary_foreground)
                    .shadow_xs()
                    .child(
                        svg()
                            .path("meeting/RefreshIcon.svg")
                            .size_4()
                            .text_color(colors.primary_foreground),
                    )
            } else {
                button
                    .border_1()
                    .border_color(colors.input)
                    .bg(colors.background)
                    .text_color(colors.foreground)
                    .shadow_xs()
            }
        })
        .child(label)
}

impl Render for TranscriptView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let dark = colors.background.l < 0.5;
        let live = self.live(cx);
        if !self.loaded && self.error.is_none() {
            return div().id("note-transcript").size_full();
        }
        let has_words = self
            .segments
            .iter()
            .any(|segment| !segment.words.is_empty());
        let screen = screen(live.as_ref(), has_words, self.audio);
        let frame = div().id("note-transcript").size_full().relative();
        match screen {
            Screen::Listening { finalizing } => frame.child(if finalizing {
                status(
                    colors,
                    // Shipping's spinner is an inline-block, so the line adds its descent below.
                    div()
                        .pb(px(6.5))
                        .child(spinner(
                            "transcript-finalizing",
                            36.,
                            colors.muted_foreground,
                        ))
                        .into_any_element(),
                    "Finalizing transcript...",
                    "Transcript is still being written.",
                )
            } else {
                status(
                    colors,
                    waveform(colors),
                    "Listening...",
                    "Transcript will appear here when the first segment arrives.",
                )
            }),
            Screen::Batch {
                requested_live: true,
            } => frame.child(
                div()
                    .size_full()
                    .min_h(px(400.))
                    .pt_2()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when_some(live.and_then(|live| live.paused), |view, message| {
                        view.child(paused_notice(colors, message))
                    }),
            ),
            Screen::Batch {
                requested_live: false,
            } => frame.child(status(
                colors,
                dancing_sticks(
                    "transcript-batch",
                    live.map(|live| live.amplitude).unwrap_or_default(),
                    rgb(0xa3a3a3).into(),
                    (80., 36.),
                    (3., 3.),
                )
                .into_any_element(),
                "Batch transcription mode",
                "Recording continues. Your transcript will be generated after you stop.",
            )),
            Screen::Empty { audio } => frame.child(
                status(
                    colors,
                    waveform(colors),
                    if audio {
                        "Audio available"
                    } else {
                        "No transcript available"
                    },
                    if audio {
                        "Re-transcribe this audio, or upload a transcript file."
                    } else {
                        "Upload audio or a transcript file to populate this note."
                    },
                )
                .child(
                    div()
                        .mt_6()
                        .flex()
                        .items_center()
                        .gap_2()
                        .map(|row| {
                            if audio {
                                row.child(disabled_button(colors, "Re-transcribe", true))
                            } else {
                                row.child(disabled_button(colors, "Upload audio", false))
                            }
                        })
                        .child(disabled_button(colors, "Upload transcript", false)),
                ),
            ),
            Screen::Ready => {
                let paused = live.as_ref().and_then(|live| live.paused.clone());
                let rows = merge(
                    &self.segments,
                    live.as_ref().map(|live| &live.segments[..]).unwrap_or(&[]),
                );
                frame.child(
                    div()
                        .id("note-transcript-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .px_1()
                        .pt_2()
                        .pb(px(80.))
                        .when_some(self.error.clone(), |view, error| {
                            view.child(
                                div()
                                    .px_2()
                                    .text_sm()
                                    .text_color(colors.muted_foreground)
                                    .child(format!("Transcript unavailable: {error}")),
                            )
                        })
                        .children(rows.iter().enumerate().map(|(index, row)| {
                            let (text, highlights) = row_text(row);
                            div().when(index > 0, |view| view.pt_4()).child(
                                div()
                                    .px_2()
                                    .rounded(px(8.))
                                    .child(
                                        div()
                                            .py_1()
                                            .text_xs()
                                            .line_height(px(16.))
                                            .font_weight(FontWeight::LIGHT)
                                            .text_color(segment_color(&row.key, dark))
                                            .child(row.label.clone()),
                                    )
                                    .child(
                                        div()
                                            .mt(px(6.))
                                            .text_sm()
                                            .line_height(px(22.75))
                                            .text_color(colors.foreground)
                                            .child(
                                                StyledText::new(text).with_highlights(highlights),
                                            ),
                                    ),
                            )
                        }))
                        .when_some(paused, |view, message| {
                            view.child(div().pt_8().child(paused_notice(colors, message)))
                        }),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_rows_render_into_speaker_segments() {
        let rows = [json!({
            "id": "t",
            "started_at_ms": 0,
            "words_json": json!([
                {"id":"a","text":"Hello","start_ms":0,"end_ms":200,"channel":0},
                {"id":"b","text":" there.","start_ms":200,"end_ms":400,"channel":0},
                {"id":"c","text":"Hi","start_ms":3000,"end_ms":3200,"channel":1}
            ]).to_string(),
            "speaker_hints_json": "[]"
        })];
        let segments = segments(&rows).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text.trim(), "Hello there.");
        assert_eq!(segments[1].key.channel, ChannelProfile::RemoteParty);
    }

    #[test]
    fn segment_colors_match_shipping_oklch_palette() {
        let key = SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: None,
            speaker_human_id: None,
        };
        // chroma.oklch(0.55, 0.15, hue).hex() in shipping.
        assert_eq!(segment_color(&key, false), Hsla::from(rgb(0xb7445d)));
        let remote = SegmentKey {
            channel: ChannelProfile::RemoteParty,
            ..key
        };
        assert_eq!(segment_color(&remote, false), Hsla::from(rgb(0x008b85)));
    }

    fn word(id: Option<&str>, text: &str, final_: bool) -> SegmentWord {
        SegmentWord {
            text: text.into(),
            start_ms: 0,
            end_ms: 0,
            channel: ChannelProfile::DirectMic,
            is_final: final_,
            id: id.map(Into::into),
        }
    }

    #[test]
    fn live_words_replace_stored_ones_and_one_speaker_stays_one_segment() {
        let key = SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: None,
            speaker_human_id: None,
        };
        let stored = vec![RenderedTranscriptSegment {
            provisional_speaker: None,
            id: "stored".into(),
            key: key.clone(),
            speaker_label: "Speaker 1".into(),
            start_ms: 0,
            end_ms: 900,
            text: "Hello there".into(),
            words: vec![
                word(Some("a"), "Hello", true),
                word(Some("b"), " there", true),
            ],
        }];
        let live = vec![Arc::new(LiveTranscriptSegment {
            id: "live".into(),
            key,
            start_ms: 500,
            end_ms: 1500,
            text: String::new(),
            words: vec![word(Some("b"), " there", true), word(None, " frie", false)],
        })];
        let rows = merge(&stored, &live);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.as_ref(), "Speaker 1");
        let (text, highlights) = row_text(&rows[0]);
        assert_eq!(text, "Hello there frie");
        assert_eq!(highlights.len(), 1);
        assert_eq!(&text[highlights[0].0.clone()], "frie");
    }

    #[test]
    fn screens_follow_shipping_capture_states() {
        let mut live = LiveMeeting::new(SessionId("s".into()), SessionMode::Active);
        assert_eq!(
            screen(Some(&live), false, false),
            Screen::Listening { finalizing: false }
        );
        live.capture = CaptureMode::RecordOnly;
        assert_eq!(
            screen(Some(&live), true, false),
            Screen::Batch {
                requested_live: false
            }
        );
        live.capture = CaptureMode::FallbackRecordOnly;
        live.paused = Some("paused".into());
        assert_eq!(
            screen(Some(&live), false, false),
            Screen::Batch {
                requested_live: true
            }
        );
        assert_eq!(screen(Some(&live), true, false), Screen::Ready);
        live.mode = SessionMode::Finalizing;
        live.paused = None;
        assert_eq!(
            screen(Some(&live), false, false),
            Screen::Listening { finalizing: true }
        );
        assert_eq!(screen(None, false, true), Screen::Empty { audio: true });
        assert_eq!(screen(None, true, false), Screen::Ready);
    }
}
