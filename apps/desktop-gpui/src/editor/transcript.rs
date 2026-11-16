//! The open note's Transcript view: shipping's `TranscriptContent` screens (listening,
//! finalizing, batch, empty) and its segment list (speaker label in its segment color, then the
//! words), rendered by the same `anlg_transcript` segmenter. While a meeting is live, stored
//! segments merge with the live ones and partial words show faded and italic.

mod speakers;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anlg_listener_core::LiveTranscriptSegment;
use anlg_transcript::{
    ChannelProfile, RenderTranscriptHuman, RenderTranscriptRequest, RenderedTranscriptSegment,
    SegmentKey, SegmentWord, SpeakerLabeler, render_transcript_segments,
};
use desktop_runtime::{RuntimeHandle, SessionId};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, FontStyle, FontWeight,
    HighlightStyle, Hsla, InteractiveText, Render, ScrollHandle, SharedString, StyledText,
    Subscription, Task, Window, div, point, prelude::*, px, rgb, rgba, svg,
};
use serde_json::{Value, json};

use crate::{
    meeting::{
        MeetingServices,
        live::{CaptureMode, LiveMeeting, LiveState, SessionMode},
        model::Transcript,
        playback::Command,
        store::TranscriptStore,
        widgets::{dancing_sticks, spinner},
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::{RADIUS, Theme, theme},
    },
};

pub struct TranscriptView {
    runtime: RuntimeHandle,
    session: SessionId,
    segments: Vec<RenderedTranscriptSegment>,
    /// Each stored word's transcript, for edits and speaker assignment.
    word_transcripts: HashMap<String, Arc<str>>,
    loaded: bool,
    audio: Option<PathBuf>,
    error: Option<String>,
    live: Option<Entity<LiveState>>,
    scroll: ScrollHandle,
    /// Shipping's transcript edit mode (the pencil on the Transcript tab).
    edit_mode: bool,
    editors: HashMap<String, SegmentEditor>,
    /// The word under the pointer, by row and word index.
    hovered: Option<(usize, usize)>,
    /// Playback position in this session's audio, for the current-line highlight.
    playing_ms: Option<i64>,
    picker: Option<speakers::SpeakerPicker>,
    focus: FocusHandle,
    _playback: Option<Task<()>>,
    _watch: Task<()>,
    _live: Option<Subscription>,
}

/// One segment's text while editing (shipping's `EditableSegmentText`).
struct SegmentEditor {
    input: Entity<TextInput>,
    original: String,
    transcript: Arc<str>,
    word_ids: Vec<String>,
    /// Bumped per keystroke; a pause in typing saves, so closing the note keeps the edit.
    revision: u64,
    _subscriptions: [Subscription; 2],
}

impl TranscriptView {
    pub fn new(runtime: RuntimeHandle, session: SessionId, cx: &mut Context<Self>) -> Self {
        // Humans named by speaker hints label their segments, as shipping's renderer does.
        let reply = runtime.watch_query(
            "SELECT t.id, t.started_at_ms, t.words_json, t.speaker_hints_json,
               (SELECT json_group_array(json_object('id', h.id, 'name', h.name)) FROM humans h
                WHERE h.deleted_at IS NULL AND instr(t.speaker_hints_json, h.id) > 0) AS humans
             FROM transcripts t
             WHERE t.session_id = ? AND t.deleted_at IS NULL ORDER BY t.started_at_ms, t.id"
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
                            Ok((segments, words)) => {
                                this.segments = segments;
                                this.word_transcripts = words;
                            }
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
            runtime,
            session,
            segments: Vec::new(),
            word_transcripts: HashMap::new(),
            loaded: false,
            audio: None,
            error: None,
            live,
            scroll: ScrollHandle::new(),
            edit_mode: false,
            editors: HashMap::new(),
            hovered: None,
            playing_ms: None,
            picker: None,
            focus: cx.focus_handle(),
            _playback: None,
            _watch: watch,
            _live: observe,
        }
    }

    /// The session's saved audio: it decides the empty screen's copy, makes words seekable
    /// and follows playback for the current line.
    pub fn set_audio(&mut self, audio: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.audio == audio {
            return;
        }
        self.audio = audio;
        self.playing_ms = None;
        self._playback = self.audio.clone().and_then(|path| {
            let playback = cx.try_global::<MeetingServices>()?.playback.clone();
            Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let snapshot = playback.snapshot();
                    let ms = (snapshot.path.as_ref() == Some(&path))
                        .then_some(snapshot.position.as_millis() as i64)
                        .filter(|ms| *ms > 0);
                    if this
                        .update(cx, |this, cx| {
                            if this.playing_ms != ms {
                                this.playing_ms = ms;
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }))
        });
        cx.notify();
    }

    pub fn edit_mode(&self) -> bool {
        self.edit_mode
    }

    /// Leaving edit mode saves every changed segment, as shipping's blur does.
    pub fn set_edit_mode(&mut self, edit_mode: bool, cx: &mut Context<Self>) {
        if self.edit_mode == edit_mode {
            return;
        }
        if !edit_mode {
            let ids: Vec<String> = self.editors.keys().cloned().collect();
            for id in ids {
                self.commit(&id, cx);
            }
            self.editors.clear();
        }
        self.edit_mode = edit_mode;
        cx.notify();
    }

    fn editor(
        &mut self,
        row: &Row,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<TextInput>> {
        if let Some(editor) = self.editors.get(&row.id) {
            return Some(editor.input.clone());
        }
        let word_ids: Vec<String> = row
            .words
            .iter()
            .filter_map(|word| word.id.clone())
            .collect();
        let transcript = self.word_transcripts.get(word_ids.first()?)?.clone();
        let original = normalize(
            &row.words
                .iter()
                .map(|word| word.text.trim())
                .collect::<Vec<_>>()
                .join(" "),
        );
        let input = cx.new(|cx| {
            let mut input = TextInput::new("", cx).inline().multiline();
            input.set_text(original.clone(), cx);
            input
        });
        let id = row.id.clone();
        let blur = cx.on_blur(&input.focus_handle(cx), window, {
            let id = id.clone();
            move |this, _, cx| this.commit(&id, cx)
        });
        // Cmd/Ctrl-Enter finishes the segment, like shipping's editor.
        let submit = cx.subscribe_in(&input, window, {
            let id = id.clone();
            move |this, _, event, window, cx| match event {
                InputEvent::Submitted => window.blur(cx),
                InputEvent::Changed => this.save_after_pause(&id, cx),
                InputEvent::Rejected => {}
            }
        });
        self.editors.insert(
            id,
            SegmentEditor {
                input: input.clone(),
                original,
                transcript,
                word_ids,
                revision: 0,
                _subscriptions: [blur, submit],
            },
        );
        Some(input)
    }

    fn save_after_pause(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(editor) = self.editors.get_mut(id) else {
            return;
        };
        editor.revision += 1;
        let revision = editor.revision;
        let id = id.to_owned();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(800))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .editors
                    .get(&id)
                    .is_some_and(|editor| editor.revision == revision)
                {
                    this.commit(&id, cx);
                }
            });
        })
        .detach();
    }

    /// Saves a segment's edited text when it changed (shipping's blur handler).
    fn commit(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(editor) = self.editors.get_mut(id) else {
            return;
        };
        let text = normalize(&editor.input.read(cx).buffer.text);
        if text == editor.original || editor.word_ids.is_empty() {
            return;
        }
        editor.original = text.clone();
        let reply = TranscriptStore(self.runtime.clone()).edit_segment_text(
            editor.transcript.clone(),
            editor.word_ids.clone(),
            text,
        );
        cx.spawn(async move |this, cx| {
            if let Err(error) = async { reply?.receive().await }.await {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("Transcript edit not saved: {error}"));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Shipping's `seekAndPlay` for a clicked word.
    fn seek(&mut self, start_ms: i64, cx: &mut Context<Self>) {
        let (Some(_), Some(services)) = (&self.audio, cx.try_global::<MeetingServices>()) else {
            return;
        };
        let playback = services.playback.clone();
        let _ = playback.send(Command::Seek(Duration::from_millis(start_ms.max(0) as u64)));
        let _ = playback.send(Command::Play);
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

type Rendered = (Vec<RenderedTranscriptSegment>, HashMap<String, Arc<str>>);

fn segments(rows: &[Value]) -> Result<Rendered, String> {
    let parse = |row: &Value, key: &str| -> Result<Vec<Value>, String> {
        serde_json::from_str(row[key].as_str().unwrap_or("[]")).map_err(|error| error.to_string())
    };
    let mut transcripts = Vec::new();
    let mut word_transcripts = HashMap::new();
    let mut humans = Vec::new();
    for row in rows {
        let id: Arc<str> = row["id"].as_str().unwrap_or_default().into();
        for human in parse(row, "humans")? {
            if let (Some(human_id), Some(name)) = (human["id"].as_str(), human["name"].as_str())
                && !humans
                    .iter()
                    .any(|known: &RenderTranscriptHuman| known.human_id == human_id)
            {
                humans.push(RenderTranscriptHuman {
                    human_id: human_id.into(),
                    name: name.into(),
                });
            }
        }
        let words = parse(row, "words_json")?
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        for word in &words {
            let word: &crate::meeting::model::Word = word;
            word_transcripts.insert(word.id.clone(), id.clone());
        }
        let transcript = Transcript {
            id: id.clone(),
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
    let segments = render_transcript_segments(RenderTranscriptRequest {
        speaker_context: None,
        preview: None,
        transcripts,
        participant_human_ids: Vec::new(),
        self_human_id: None,
        humans,
    });
    Ok((segments, word_transcripts))
}

/// Shipping's `normalizeEditableTranscriptText`.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One rendered segment: who spoke, and their words.
#[derive(Debug)]
struct Row {
    id: String,
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
            id: segment.id.clone(),
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
            id: segment.id.clone(),
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

type RowText = (
    String,
    Vec<(Range<usize>, HighlightStyle)>,
    Vec<Range<usize>>,
);

/// A segment's words joined by spaces: partial words `opacity-60 italic`, the sentence being
/// played on yellow (shipping's `data-line-current`), the hovered seekable word on the accent.
/// Also returns each word's byte range, for clicks.
fn row_text(
    row: &Row,
    current_ms: Option<i64>,
    hovered: Option<usize>,
    dark: bool,
    accent: Hsla,
) -> RowText {
    if row.words.is_empty() {
        let text = row.text.split_whitespace().collect::<Vec<_>>().join(" ");
        return (text, Vec::new(), Vec::new());
    }
    let partial = HighlightStyle {
        font_style: Some(FontStyle::Italic),
        fade_out: Some(0.4),
        ..Default::default()
    };
    let current_line = HighlightStyle {
        background_color: Some(
            if dark {
                rgba(0x713f124d)
            } else {
                rgba(0xfef9c380)
            }
            .into(),
        ),
        ..Default::default()
    };
    let mut text = String::new();
    let mut highlights = Vec::new();
    let mut ranges = Vec::new();
    for line in sentence_lines(&row.words) {
        let start = text.len();
        let words = &row.words[line.clone()];
        for word in words {
            if !text.is_empty() {
                text.push(' ');
            }
            let begin = text.len();
            text.push_str(word.text.trim());
            ranges.push(begin..text.len());
        }
        let first = words.first().map_or(0, |word| word.start_ms);
        let last = words.last().map_or(0, |word| word.end_ms);
        if current_ms.is_some_and(|ms| ms >= first && ms <= last) {
            highlights.push((start..text.len(), current_line));
        }
    }
    for (index, word) in row.words.iter().enumerate() {
        if !word.is_final {
            highlights.push((ranges[index].clone(), partial));
        }
    }
    if let Some(index) = hovered.filter(|index| seekable(&row.words[*index])) {
        highlights.push((
            ranges[index].clone(),
            HighlightStyle {
                background_color: Some(accent.opacity(0.6)),
                ..Default::default()
            },
        ));
    }
    (text, highlights, ranges)
}

/// Shipping's `groupWordsIntoLines`: sentences end at `.`, `?` or `!`.
fn sentence_lines(words: &[SegmentWord]) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, word) in words.iter().enumerate() {
        if word.text.trim().ends_with(['.', '?', '!']) {
            lines.push(start..index + 1);
            start = index + 1;
        }
    }
    if start < words.len() {
        lines.push(start..words.len());
    }
    lines
}

/// Shipping's `isTranscriptWordSeekable` rejects only synthetic text timing, which rendered
/// segments do not carry; partial live words are never played back.
fn seekable(word: &SegmentWord) -> bool {
    word.is_final
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
                        .font_weight(crate::ui::theme::MEDIUM)
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
        .font_weight(crate::ui::theme::MEDIUM)
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
        let screen = screen(live.as_ref(), has_words, self.audio.is_some());
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
                // Shipping edits and seeks only once the meeting is over.
                let editing = self.edit_mode && live.is_none();
                let audio = self.audio.is_some() && live.is_none();
                let mut children = Vec::with_capacity(rows.len());
                for (index, row) in rows.iter().enumerate() {
                    let editor = if editing {
                        self.editor(row, window, cx)
                    } else {
                        None
                    };
                    let header = self.segment_header(index, row, editing, dark, colors, cx);
                    let body = div()
                        .mt(px(6.))
                        .text_sm()
                        .line_height(px(22.75))
                        .text_color(colors.foreground);
                    let body = match editor {
                        Some(input) => body.child(input).into_any_element(),
                        None => body
                            .child(self.words(index, row, audio, colors, cx))
                            .into_any_element(),
                    };
                    children.push(
                        div()
                            .when(index > 0, |view| view.pt_4())
                            .child(div().px_2().rounded(px(8.)).child(header).child(body))
                            .into_any_element(),
                    );
                }
                frame
                    .child(
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
                            .children(children)
                            .when_some(paused, |view, message| {
                                view.child(div().pt_8().child(paused_notice(colors, message)))
                            }),
                    )
                    .children(self.scroll_controls(colors, cx))
            }
        }
    }
}

impl TranscriptView {
    /// Shipping's `SegmentHeader`: the select circle while editing, then the speaker label,
    /// which opens the speaker picker.
    fn segment_header(
        &self,
        index: usize,
        row: &Row,
        editing: bool,
        dark: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let color = segment_color(&row.key, dark);
        let open = self
            .picker
            .as_ref()
            .is_some_and(|picker| picker.segment == row.id);
        let target = speakers::Target::new(row, &self.word_transcripts);
        div()
            .relative()
            .py_1()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .line_height(px(16.))
            .font_weight(FontWeight::LIGHT)
            .when(editing, |header| {
                header.child(
                    div()
                        .size_4()
                        .flex_shrink_0()
                        .rounded_full()
                        .border_1()
                        .border_color(colors.muted_foreground.opacity(0.4)),
                )
            })
            .child(
                div()
                    .relative()
                    .child(
                        div()
                            .id(("speaker", index))
                            .my(px(-2.))
                            .py(px(2.))
                            .pr_2()
                            .cursor_pointer()
                            .text_color(color)
                            .when(open, |label| label.underline())
                            .hover(|style| style.underline())
                            .when_some(target, |label, target| {
                                label.on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_picker(target.clone(), window, cx)
                                }))
                            })
                            .child(row.label.clone()),
                    )
                    .when(open, |view| {
                        view.children(self.speaker_picker(colors, cx).map(|picker| {
                            div().absolute().top(px(-2.)).left_full().child(
                                gpui::deferred(gpui::anchored().snap_to_window().child(picker))
                                    .with_priority(1),
                            )
                        }))
                    }),
            )
            .into_any_element()
    }

    /// The segment's words; with audio, each seeks playback (shipping's `WordSpan`).
    fn words(
        &self,
        index: usize,
        row: &Row,
        audio: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hovered = self
            .hovered
            .filter(|(row, _)| audio && *row == index)
            .map(|(_, word)| word);
        let current = self.playing_ms.filter(|_| audio);
        let dark = colors.background.l < 0.5;
        let (text, highlights, ranges) = row_text(row, current, hovered, dark, colors.accent);
        let text = StyledText::new(text).with_highlights(highlights);
        if !audio || ranges.is_empty() {
            return text.into_any_element();
        }
        let starts: Vec<i64> = row.words.iter().map(|word| word.start_ms).collect();
        let view = cx.entity().downgrade();
        let hover_view = view.clone();
        let word_ranges = ranges.clone();
        InteractiveText::new(("transcript-words", index), text)
            .on_click(ranges, move |word, _, cx| {
                let start = starts[word];
                let _ = view.update(cx, |this, cx| this.seek(start, cx));
            })
            .on_hover(move |position, _, _, cx| {
                let word = position.and_then(|position| {
                    word_ranges
                        .iter()
                        .position(|range| range.contains(&position))
                });
                let _ = hover_view.update(cx, |this, cx| {
                    let hovered = word.map(|word| (index, word));
                    if this.hovered != hovered {
                        this.hovered = hovered;
                        cx.notify();
                    }
                });
            })
            .into_any_element()
    }

    /// Shipping's scroll-to-top and scroll-to-bottom controls, shown once the transcript scrolls.
    fn scroll_controls(&self, colors: Theme, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let max = self.scroll.max_offset().y;
        if max <= px(0.) {
            return None;
        }
        let offset = -self.scroll.offset().y;
        let (at_top, at_bottom) = (offset <= px(0.), offset >= max);
        let button = |id: &'static str, path: &'static str, disabled: bool| {
            div()
                .id(id)
                .size_8()
                .flex()
                .items_center()
                .justify_center()
                .when_else(
                    disabled,
                    |button| button.opacity(0.3),
                    move |button| {
                        button
                            .cursor_pointer()
                            .hover(move |style| style.bg(colors.muted.opacity(0.55)))
                    },
                )
                .child(
                    svg()
                        .path(path)
                        .size(px(14.))
                        .text_color(colors.muted_foreground.opacity(0.45))
                        .group_hover("transcript-scroll", move |style| {
                            style.text_color(colors.foreground)
                        }),
                )
        };
        Some(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .right_1()
                .flex()
                .items_center()
                .child(
                    div()
                        .id("transcript-scroll-controls")
                        .group("transcript-scroll")
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .rounded(px(RADIUS))
                        .border_1()
                        .border_color(gpui::transparent_black())
                        .hover(move |style| {
                            style
                                .border_color(colors.border.opacity(0.5))
                                .bg(colors.background.opacity(0.65))
                                .shadow_sm()
                        })
                        .child(
                            button("transcript-scroll-top", "editor/ArrowUp02Icon.svg", at_top)
                                .when(!at_top, |button| {
                                    button.on_click(cx.listener(|this, _, _, cx| {
                                        this.scroll.set_offset(point(px(0.), px(0.)));
                                        cx.notify();
                                    }))
                                }),
                        )
                        .child(
                            div()
                                .h(px(1.))
                                .w_full()
                                .bg(colors.border.opacity(0.2))
                                .group_hover("transcript-scroll", move |style| {
                                    style.bg(colors.border.opacity(0.6))
                                }),
                        )
                        .child(
                            button(
                                "transcript-scroll-bottom",
                                "editor/ArrowDown02Icon.svg",
                                at_bottom,
                            )
                            .when(!at_bottom, |button| {
                                button.on_click(cx.listener(|this, _, _, cx| {
                                    this.scroll.scroll_to_bottom();
                                    cx.notify();
                                }))
                            }),
                        ),
                ),
        )
    }
}

impl Focusable for TranscriptView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
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
        let (segments, words) = segments(&rows).unwrap();
        assert_eq!(words.get("c").map(AsRef::as_ref), Some("t"));
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
        let (text, highlights, _) = row_text(&rows[0], None, None, false, gpui::black());
        assert_eq!(text, "Hello there frie");
        assert_eq!(highlights.len(), 1);
        assert_eq!(&text[highlights[0].0.clone()], "frie");
    }

    #[test]
    fn the_played_sentence_is_highlighted_and_words_keep_their_ranges() {
        let key = SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: None,
            speaker_human_id: None,
        };
        let timed = |id: &str, text: &str, start_ms: i64, end_ms: i64| SegmentWord {
            start_ms,
            end_ms,
            ..word(Some(id), text, true)
        };
        let row = Row {
            id: "s".into(),
            key,
            label: "Speaker 1".into(),
            start_ms: 0,
            words: vec![
                timed("a", "One.", 0, 100),
                timed("b", " Two", 200, 300),
                timed("c", " three.", 300, 400),
            ],
            text: String::new(),
        };
        let (text, highlights, ranges) = row_text(&row, Some(250), Some(0), false, gpui::black());
        assert_eq!(text, "One. Two three.");
        assert_eq!(&text[ranges[1].clone()], "Two");
        let playing = highlights
            .iter()
            .find(|(_, style)| style.background_color.is_some_and(|color| color.a < 0.6))
            .map(|(range, _)| &text[range.clone()]);
        // Shipping's line span holds the space before it too.
        assert_eq!(playing, Some(" Two three."));
        assert_eq!(
            highlights.len(),
            2,
            "the played sentence and the hovered word"
        );
        assert_eq!(normalize("  Hi \n there "), "Hi there");
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
