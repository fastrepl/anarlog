//! Transcript rendering data: the port of `apps/desktop/src/stt/render-transcript.ts`
//! (`buildRenderTranscriptRequest`) feeding the shared `anlg-transcript`
//! segmenter, plus the per-speaker colours from
//! `note-input/transcript/renderer/utils.ts`.

use anlg_transcript::{
    ChannelProfile, IdentityAssignment, IdentityScope, RenderTranscriptHuman,
    RenderTranscriptInput, RenderTranscriptRequest, RenderTranscriptWordInput, SegmentKey,
    render_transcript_segments,
};
use serde_json::Value;

use crate::db::TranscriptRow;

/// One stored transcript, segmented the way the Transcript tab shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedTranscript {
    pub id: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub segments: Vec<Segment>,
}

/// The parts of `RenderedTranscriptSegment` the view draws.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub id: String,
    pub key: SegmentKey,
    pub speaker_label: String,
    pub start_ms: i64,
    pub end_ms: i64,
    /// Words joined by single spaces (`getWordDisplayText` + the line joins).
    pub text: String,
    /// `WordSpan` per word, with its byte range in `text`.
    pub words: Vec<Word>,
    /// `groupWordsIntoLines`: sentence lines (closing on `.`, `?`, `!`).
    pub lines: Vec<Line>,
    /// The segmenter's own label and text, which the export uses
    /// (`buildTranscriptExportSegments` reads `speaker_label` / `text`).
    pub export_speaker: String,
    pub export_text: String,
}

/// One `WordSpan`.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub id: Option<String>,
    /// `getWordDisplayText`: the trimmed text.
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub is_final: bool,
    /// `isTranscriptWordSeekable`: not a `synthetic_text` timing.
    pub seekable: bool,
    /// Byte range of the word inside [`Segment::text`].
    pub range: std::ops::Range<usize>,
}

/// One `SentenceLine`, as indexes into [`Segment::words`].
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub words: std::ops::Range<usize>,
    pub start_ms: i64,
    pub end_ms: i64,
}

impl Segment {
    /// `getActiveLineIndex`: the line whose `[offset + start, offset + end]`
    /// window holds `current_ms` (none at or before 0).
    pub fn active_line(&self, offset_ms: i64, current_ms: i64) -> Option<usize> {
        if current_ms <= 0 {
            return None;
        }
        self.lines.iter().position(|line| {
            current_ms >= offset_ms + line.start_ms && current_ms <= offset_ms + line.end_ms
        })
    }

    /// The byte range of `line` in [`Segment::text`].
    pub fn line_range(&self, line: usize) -> Option<std::ops::Range<usize>> {
        let line = self.lines.get(line)?;
        let first = self.words.get(line.words.start)?;
        let last = self.words.get(line.words.end.checked_sub(1)?)?;
        Some(first.range.start..last.range.end)
    }

    /// The word under a byte offset of [`Segment::text`], if any.
    pub fn word_at(&self, offset: usize) -> Option<usize> {
        self.words
            .iter()
            .position(|word| word.range.start <= offset && offset < word.range.end)
    }
}

/// `groupWordsIntoLines` over the display texts: a line closes after a word
/// ending in `.`, `?` or `!`, and the tail forms the last line.
pub fn group_words_into_lines(words: &[Word]) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, word) in words.iter().enumerate() {
        let closes =
            word.text.ends_with('.') || word.text.ends_with('?') || word.text.ends_with('!');
        if closes || index + 1 == words.len() {
            lines.push(Line {
                words: start..index + 1,
                start_ms: words[start].start_ms,
                end_ms: word.end_ms,
            });
            start = index + 1;
        }
    }
    lines
}

/// `getTranscriptTimelineOffsetMs`: this transcript's start relative to the
/// earliest transcript with words in the session (else the earliest at all).
pub fn timeline_offset_ms(started_at_ms: i64, transcripts: &[(i64, bool)]) -> i64 {
    let candidates: Vec<&(i64, bool)> = transcripts
        .iter()
        .filter(|(started, _)| *started > 0)
        .collect();
    let with_words: Vec<&&(i64, bool)> = candidates.iter().filter(|(_, has)| *has).collect();
    let earliest = if with_words.is_empty() {
        candidates.iter().map(|(started, _)| *started).min()
    } else {
        with_words.iter().map(|(started, _)| *started).min()
    };
    match earliest {
        Some(earliest) => (started_at_ms - earliest).max(0),
        None => 0,
    }
}

/// `getTranscriptTimingSource(word) !== "synthetic_text"` over the stored
/// word's `metadata`, which `normalizeWordMetadata` also accepts as a JSON
/// string.
fn word_is_seekable(word: &Value) -> bool {
    let parsed = match word.get("metadata") {
        Some(Value::String(json)) => serde_json::from_str::<Value>(json).ok(),
        Some(value) => Some(value.clone()),
        None => None,
    };
    let Some(metadata) = parsed.filter(|m| m.is_object()) else {
        return true;
    };
    let source = metadata
        .get("timing")
        .filter(|t| t.is_object())
        .and_then(|timing| timing.get("source"))
        .and_then(Value::as_str)
        .filter(|source| is_valid_timing_source(source))
        .or_else(|| {
            metadata
                .get("timing_source")
                .and_then(Value::as_str)
                .filter(|source| is_valid_timing_source(source))
        });
    source != Some("synthetic_text")
}

fn is_valid_timing_source(source: &str) -> bool {
    matches!(
        source,
        "provider_word" | "provider_segment_interpolated" | "synthetic_text"
    )
}

/// `useRenderedTranscriptData(transcriptId)`: each transcript is rendered on
/// its own, with the session participants and every assigned human as the
/// label context.
pub fn render_transcripts(
    rows: &[TranscriptRow],
    participant_human_ids: &[String],
    humans: &[(String, String)],
) -> Vec<RenderedTranscript> {
    let self_human_id = rows
        .first()
        .map(|row| row.owner_user_id.clone())
        .filter(|id| !id.is_empty());
    let humans: Vec<RenderTranscriptHuman> = humans
        .iter()
        .map(|(human_id, name)| RenderTranscriptHuman {
            human_id: human_id.clone(),
            name: name.clone(),
        })
        .collect();
    rows.iter()
        .filter_map(|row| {
            let request = build_render_request(
                std::slice::from_ref(row),
                participant_human_ids,
                self_human_id.as_deref(),
                &humans,
            )?;
            let context = LabelContext {
                self_human_id: request.self_human_id.clone(),
                participant_human_ids: request.participant_human_ids.clone(),
                names: request
                    .humans
                    .iter()
                    .map(|human| (human.human_id.clone(), human.name.clone()))
                    .collect(),
            };
            let non_seekable: std::collections::HashSet<String> = parse_array(&row.words_json)
                .iter()
                .filter(|word| !word_is_seekable(word))
                .filter_map(|word| word.get("id").and_then(Value::as_str).map(str::to_string))
                .collect();
            let rendered = render_transcript_segments(request);
            // `SpeakerLabelManager.fromSegments` numbers the unknown speakers
            // in order of appearance before any label is rendered.
            let mut labels = SpeakerLabelManager::new(max_speaker_number(
                &context.participant_human_ids,
                context.self_human_id.as_deref(),
            ));
            for segment in &rendered {
                if !context.is_known_speaker(&segment.key) {
                    labels.unknown_speaker_number(&segment.key);
                }
            }
            Some(RenderedTranscript {
                id: row.id.clone(),
                started_at_ms: row.started_at_ms,
                ended_at_ms: row.ended_at_ms,
                segments: rendered
                    .into_iter()
                    .map(|segment| {
                        let mut text = String::new();
                        let mut words = Vec::with_capacity(segment.words.len());
                        for word in &segment.words {
                            let display = word.text.trim();
                            if display.is_empty() {
                                continue;
                            }
                            if !text.is_empty() {
                                text.push(' ');
                            }
                            let start = text.len();
                            text.push_str(display);
                            words.push(Word {
                                seekable: word
                                    .id
                                    .as_ref()
                                    .is_none_or(|id| !non_seekable.contains(id)),
                                id: word.id.clone(),
                                text: display.to_string(),
                                start_ms: word.start_ms,
                                end_ms: word.end_ms,
                                is_final: word.is_final,
                                range: start..text.len(),
                            });
                        }
                        let lines = group_words_into_lines(&words);
                        Segment {
                            id: segment.id,
                            speaker_label: render_label(&segment.key, &context, &mut labels),
                            key: segment.key,
                            start_ms: segment.start_ms,
                            end_ms: segment.end_ms,
                            text,
                            words,
                            lines,
                            export_speaker: segment.speaker_label,
                            export_text: segment.text,
                        }
                    })
                    .collect(),
            })
        })
        .collect()
}

/// `RenderLabelContext` from `~/stt/live-segment`.
struct LabelContext {
    self_human_id: Option<String>,
    participant_human_ids: Vec<String>,
    names: std::collections::HashMap<String, String>,
}

impl LabelContext {
    fn name(&self, human_id: &str) -> Option<&str> {
        self.names
            .get(human_id)
            .map(String::as_str)
            .filter(|name| !name.is_empty())
    }

    /// `getUniqueRemoteParticipantHumanId`
    fn unique_remote_participant(&self) -> Option<&str> {
        let mut remote: Vec<&str> = self
            .participant_human_ids
            .iter()
            .map(String::as_str)
            .filter(|id| !id.is_empty() && Some(*id) != self.self_human_id.as_deref())
            .collect();
        remote.sort_unstable();
        remote.dedup();
        match remote.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }

    /// `SegmentKeyUtils.isKnownSpeaker`
    fn is_known_speaker(&self, key: &SegmentKey) -> bool {
        if key.speaker_human_id.is_some() {
            return true;
        }
        match key.channel {
            ChannelProfile::DirectMic => self.self_human_id.is_some(),
            ChannelProfile::RemoteParty => self.unique_remote_participant().is_some(),
            ChannelProfile::MixedCapture => false,
        }
    }
}

/// `SpeakerLabelManager`
struct SpeakerLabelManager {
    unknown: Vec<(SegmentKey, usize)>,
    next_index: usize,
    max_unknown_speaker_number: Option<usize>,
}

impl SpeakerLabelManager {
    fn new(max_unknown_speaker_number: Option<usize>) -> Self {
        Self {
            unknown: Vec::new(),
            next_index: 1,
            max_unknown_speaker_number,
        }
    }

    fn unknown_speaker_number(&mut self, key: &SegmentKey) -> usize {
        if let Some((_, number)) = self.unknown.iter().find(|(k, _)| k == key) {
            return *number;
        }
        let number = match self.max_unknown_speaker_number {
            Some(max) if max > 0 => self.next_index.min(max),
            _ => self.next_index,
        };
        self.unknown.push((key.clone(), number));
        self.next_index += 1;
        number
    }
}

/// `getMaxSpeakerNumberForParticipants`
fn max_speaker_number(
    participant_human_ids: &[String],
    self_human_id: Option<&str>,
) -> Option<usize> {
    let mut ids: Vec<&str> = participant_human_ids
        .iter()
        .map(String::as_str)
        .filter(|id| !id.is_empty())
        .collect();
    if let Some(self_id) = self_human_id.filter(|id| !id.is_empty()) {
        ids.push(self_id);
    }
    ids.sort_unstable();
    ids.dedup();
    (ids.len() > 1).then_some(ids.len())
}

/// `SegmentKeyUtils.renderLabel`
fn render_label(key: &SegmentKey, ctx: &LabelContext, manager: &mut SpeakerLabelManager) -> String {
    if let Some(human_id) = key.speaker_human_id.as_deref()
        && let Some(name) = ctx.name(human_id)
    {
        return name.to_string();
    }
    if key.channel == ChannelProfile::DirectMic
        && key.speaker_human_id.is_none()
        && let Some(self_id) = ctx.self_human_id.as_deref()
    {
        return ctx.name(self_id).unwrap_or("You").to_string();
    }
    if key.channel == ChannelProfile::RemoteParty
        && key.speaker_human_id.is_none()
        && let Some(remote) = ctx.unique_remote_participant()
    {
        return ctx.name(remote).unwrap_or(remote).to_string();
    }
    format!("Speaker {}", manager.unknown_speaker_number(key))
}

/// `collectAssignedHumanIdsFromTranscriptRows`
pub fn assigned_human_ids(rows: &[TranscriptRow]) -> Vec<String> {
    let mut ids = Vec::new();
    for row in rows {
        for hint in parse_array(&row.speaker_hints_json) {
            let kind = hint.get("type").and_then(Value::as_str).unwrap_or("");
            if kind != "automatic_speaker_assignment" && kind != "user_speaker_assignment" {
                continue;
            }
            if let Some(human_id) = hint
                .get("value")
                .and_then(parse_hint_value)
                .and_then(|value| value.get("human_id")?.as_str().map(str::to_string))
                .filter(|id| !id.is_empty())
                && !ids.contains(&human_id)
            {
                ids.push(human_id);
            }
        }
    }
    ids
}

/// `buildRenderTranscriptRequest`
pub fn build_render_request(
    rows: &[TranscriptRow],
    participant_human_ids: &[String],
    self_human_id: Option<&str>,
    humans: &[RenderTranscriptHuman],
) -> Option<RenderTranscriptRequest> {
    let mut transcripts = Vec::new();
    for row in rows {
        let mut words: Vec<RenderTranscriptWordInput> = Vec::new();
        let mut index_by_id = std::collections::HashMap::new();
        for word in parse_array(&row.words_json) {
            let (Some(id), Some(text), Some(start_ms), Some(end_ms)) = (
                word.get("id").and_then(Value::as_str),
                word.get("text").and_then(Value::as_str),
                word.get("start_ms").and_then(Value::as_f64),
                word.get("end_ms").and_then(Value::as_f64),
            ) else {
                continue;
            };
            index_by_id.insert(id.to_string(), words.len());
            words.push(RenderTranscriptWordInput {
                id: id.to_string(),
                text: text.to_string(),
                start_ms: start_ms as i64,
                end_ms: end_ms as i64,
                channel: word
                    .get("channel")
                    .and_then(Value::as_i64)
                    .map(|channel| channel as i32)
                    .unwrap_or(0),
                speaker_index: None,
            });
        }

        let hints = parse_array(&row.speaker_hints_json);
        let mut assignments = Vec::new();
        // Provider speaker indexes first, then automatic and user assignments,
        // in the same passes as the web app.
        for pass in [
            "provider_speaker_index",
            "automatic_speaker_assignment",
            "user_speaker_assignment",
        ] {
            for hint in &hints {
                if hint.get("type").and_then(Value::as_str) != Some(pass) {
                    continue;
                }
                if let Some(assignment) = normalize_speaker_hint(hint, &mut words, &index_by_id) {
                    assignments.push(assignment);
                }
            }
        }

        if words.is_empty() {
            continue;
        }
        transcripts.push(RenderTranscriptInput {
            started_at: Some(row.started_at_ms),
            words,
            assignments,
        });
    }
    if transcripts.is_empty() {
        return None;
    }
    Some(RenderTranscriptRequest {
        transcripts,
        participant_human_ids: participant_human_ids.to_vec(),
        self_human_id: self_human_id.map(str::to_string),
        humans: humans.to_vec(),
    })
}

fn channel_profile(channel: i32) -> ChannelProfile {
    match channel {
        0 => ChannelProfile::DirectMic,
        1 => ChannelProfile::RemoteParty,
        _ => ChannelProfile::MixedCapture,
    }
}

/// `normalizeSpeakerHint`
fn normalize_speaker_hint(
    hint: &Value,
    words: &mut [RenderTranscriptWordInput],
    index_by_id: &std::collections::HashMap<String, usize>,
) -> Option<IdentityAssignment> {
    let word_id = hint.get("word_id").and_then(Value::as_str)?;
    let kind = hint.get("type").and_then(Value::as_str)?;
    let value = hint.get("value").and_then(parse_hint_value)?;
    if !value.is_object() {
        return None;
    }

    let is_assignment = kind == "automatic_speaker_assignment" || kind == "user_speaker_assignment";
    let human_id = value.get("human_id").and_then(Value::as_str);
    if is_assignment && let Some(human_id) = human_id {
        if let Some(scope) = explicit_speaker_scope(&value) {
            return Some(IdentityAssignment {
                human_id: human_id.to_string(),
                scope,
            });
        }
        if value.get("scope").and_then(Value::as_str) == Some("segment")
            && let Some(word_ids) = value.get("word_ids").and_then(Value::as_array)
        {
            let word_ids: Vec<String> = word_ids
                .iter()
                .filter_map(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect();
            if !word_ids.is_empty() {
                return Some(IdentityAssignment {
                    human_id: human_id.to_string(),
                    scope: IdentityScope::Words { word_ids },
                });
            }
        }
    }

    let word = words.get_mut(*index_by_id.get(word_id)?)?;

    if kind == "provider_speaker_index"
        && let Some(speaker_index) = value.get("speaker_index").and_then(Value::as_i64)
    {
        word.speaker_index = Some(speaker_index as i32);
        if let Some(channel) = value.get("channel").and_then(Value::as_i64) {
            word.channel = channel as i32;
        }
        return None;
    }

    if is_assignment && let Some(human_id) = human_id {
        let channel = channel_profile(word.channel);
        return Some(IdentityAssignment {
            human_id: human_id.to_string(),
            scope: match word.speaker_index {
                None => IdentityScope::Channel { channel },
                Some(speaker_index) => IdentityScope::ChannelSpeaker {
                    channel,
                    speaker_index,
                },
            },
        });
    }

    None
}

/// `getExplicitSpeakerScope`
fn explicit_speaker_scope(value: &Value) -> Option<IdentityScope> {
    if value.get("scope").and_then(Value::as_str) != Some("speaker") {
        return None;
    }
    let channel = value.get("channel").and_then(Value::as_i64)?;
    if !(0..=2).contains(&channel) {
        return None;
    }
    let channel = channel_profile(channel as i32);
    match value.get("speaker_index") {
        Some(Value::Number(speaker_index)) => Some(IdentityScope::ChannelSpeaker {
            channel,
            speaker_index: speaker_index.as_i64()? as i32,
        }),
        Some(Value::Null) => Some(IdentityScope::Channel { channel }),
        _ => None,
    }
}

/// `parseHintValue`: values are stored either as objects or JSON strings.
fn parse_hint_value(value: &Value) -> Option<Value> {
    match value {
        Value::String(json) => serde_json::from_str(json).ok(),
        other => Some(other.clone()),
    }
}

fn parse_array(json: &str) -> Vec<Value> {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
}

/// `getSegmentColor`: `chroma.oklch(0.55 | 0.72, 0.15, hue)` with the hue
/// picked per channel palette and speaker index.
pub fn segment_color(key: &SegmentKey, dark: bool) -> gpui::Rgba {
    let speaker_index = key.speaker_index.unwrap_or(0).max(0) as usize;
    let hues: [f64; 6] = if key.channel == ChannelProfile::RemoteParty {
        [285.0, 305.0, 270.0, 295.0, 315.0, 280.0]
    } else {
        [10.0, 25.0, 0.0, 340.0, 15.0, 350.0]
    };
    let hue = hues[speaker_index % hues.len()];
    let (r, g, b) = oklch_to_srgb(if dark { 0.72 } else { 0.55 }, 0.15, hue);
    gpui::Rgba {
        r: r as f32,
        g: g as f32,
        b: b as f32,
        a: 1.0,
    }
}

/// OKLCH -> sRGB, clipped per channel like chroma-js's `.hex()`.
fn oklch_to_srgb(l: f64, c: f64, h_deg: f64) -> (f64, f64, f64) {
    let h = h_deg.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3;
    let g = -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3;
    let b = -0.004_196_086_3 * l3 - 0.703_418_614_8 * m3 + 1.707_614_701_0 * s3;
    let gamma = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    (gamma(r), gamma(g), gamma(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(words: Value, hints: Value) -> TranscriptRow {
        TranscriptRow {
            id: "t1".into(),
            owner_user_id: String::new(),
            started_at_ms: 0,
            ended_at_ms: None,
            words_json: words.to_string(),
            speaker_hints_json: hints.to_string(),
            pending_deltas_json: "[]".into(),
        }
    }

    #[test]
    fn provider_speaker_indexes_split_segments_and_label_speakers() {
        let words = serde_json::json!([
            {"id": "w1", "text": "hello", "start_ms": 0, "end_ms": 400, "channel": 0},
            {"id": "w2", "text": "there", "start_ms": 400, "end_ms": 800, "channel": 0},
            {"id": "w3", "text": "hi", "start_ms": 900, "end_ms": 1200, "channel": 0}
        ]);
        let hints = serde_json::json!([
            {"id": "h1", "word_id": "w1", "type": "provider_speaker_index", "value": {"speaker_index": 0}},
            {"id": "h2", "word_id": "w2", "type": "provider_speaker_index", "value": {"speaker_index": 0}},
            {"id": "h3", "word_id": "w3", "type": "provider_speaker_index", "value": "{\"speaker_index\": 1}"}
        ]);
        let rendered = render_transcripts(&[row(words, hints)], &[], &[]);
        assert_eq!(rendered.len(), 1);
        let segments = &rendered[0].segments;
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "hello there");
        assert_eq!(segments[1].text, "hi");
        assert_ne!(segments[0].speaker_label, segments[1].speaker_label);
        assert_eq!(segments[0].key.speaker_index, Some(0));
        assert_eq!(segments[1].key.speaker_index, Some(1));
    }

    #[test]
    fn words_carry_ranges_lines_and_seekability() {
        let words = serde_json::json!([
            {"id": "w1", "text": " Hello ", "start_ms": 0, "end_ms": 400, "channel": 0},
            {"id": "w2", "text": "there.", "start_ms": 400, "end_ms": 800, "channel": 0},
            {"id": "w3", "text": "Next", "start_ms": 900, "end_ms": 1200, "channel": 0,
             "metadata": "{\"timing\":{\"source\":\"synthetic_text\"}}"},
            {"id": "w4", "text": "one!", "start_ms": 1200, "end_ms": 1500, "channel": 0,
             "metadata": {"timing_source": "provider_segment_interpolated"}},
            {"id": "w5", "text": "Tail", "start_ms": 1600, "end_ms": 1900, "channel": 0}
        ]);
        let rendered = render_transcripts(&[row(words, serde_json::json!([]))], &[], &[]);
        let segment = &rendered[0].segments[0];
        assert_eq!(segment.text, "Hello there. Next one! Tail");
        assert_eq!(segment.words.len(), 5);
        assert_eq!(&segment.text[segment.words[1].range.clone()], "there.");
        assert_eq!(&segment.text[segment.words[4].range.clone()], "Tail");
        assert!(segment.words[0].seekable);
        assert!(!segment.words[2].seekable);
        assert!(segment.words[3].seekable);
        assert_eq!(segment.lines.len(), 3);
        assert_eq!(segment.lines[0].words, 0..2);
        assert_eq!(segment.lines[1].words, 2..4);
        assert_eq!(segment.lines[2].words, 4..5);
        assert_eq!(
            (segment.lines[1].start_ms, segment.lines[1].end_ms),
            (900, 1500)
        );
        assert_eq!(segment.line_range(0), Some(0..12));
        assert_eq!(segment.active_line(0, 0), None);
        assert_eq!(segment.active_line(0, 1000), Some(1));
        assert_eq!(segment.active_line(500, 1000), Some(0));
        assert_eq!(segment.active_line(0, 1550), None);
        assert_eq!(segment.word_at(7), Some(1));
        assert_eq!(segment.word_at(12), None);
        assert_eq!(
            timeline_offset_ms(5000, &[(2000, true), (1000, false), (5000, true)]),
            3000
        );
        assert_eq!(timeline_offset_ms(5000, &[(6000, false), (0, true)]), 0);
        assert_eq!(timeline_offset_ms(5000, &[]), 0);
    }

    #[test]
    fn user_assignments_name_the_speaker() {
        let words = serde_json::json!([
            {"id": "w1", "text": "hello", "start_ms": 0, "end_ms": 400, "channel": 0}
        ]);
        let hints = serde_json::json!([
            {"id": "h1", "word_id": "w1", "type": "provider_speaker_index", "value": {"speaker_index": 0}},
            {"id": "h2", "word_id": "w1", "type": "user_speaker_assignment", "value": {"human_id": "alice"}}
        ]);
        let rows = [row(words, hints)];
        assert_eq!(assigned_human_ids(&rows), vec!["alice".to_string()]);
        let rendered = render_transcripts(&rows, &[], &[("alice".into(), "Alice".into())]);
        assert_eq!(rendered[0].segments[0].speaker_label, "Alice");
        assert_eq!(
            rendered[0].segments[0].key.speaker_human_id.as_deref(),
            Some("alice")
        );
    }

    #[test]
    fn an_unnamed_owner_is_numbered_like_the_web_view() {
        let words = serde_json::json!([
            {"id": "w1", "text": "hello", "start_ms": 0, "end_ms": 400, "channel": 0}
        ]);
        let mut row = row(words, serde_json::json!([]));
        row.owner_user_id = "owner".into();
        let rendered = render_transcripts(&[row], &[], &[]);
        assert_eq!(
            rendered[0].segments[0].key.speaker_human_id.as_deref(),
            Some("owner")
        );
        assert_eq!(rendered[0].segments[0].speaker_label, "Speaker 1");
    }

    #[test]
    fn words_without_ids_are_dropped_and_empty_transcripts_skipped() {
        let words = serde_json::json!([{"text": "orphan", "start_ms": 0, "end_ms": 1}]);
        assert!(render_transcripts(&[row(words, serde_json::json!([]))], &[], &[]).is_empty());
    }

    #[test]
    fn segment_colors_match_chroma_oklch() {
        // Reference values from `chroma.oklch(l, 0.15, h).hex()` (chroma-js 3.2).
        let hex = |channel, speaker_index, dark| {
            let color = segment_color(
                &SegmentKey {
                    channel,
                    speaker_index: Some(speaker_index),
                    speaker_human_id: None,
                },
                dark,
            );
            format!(
                "#{:02x}{:02x}{:02x}",
                (color.r * 255.0).round() as u8,
                (color.g * 255.0).round() as u8,
                (color.b * 255.0).round() as u8
            )
        };
        assert_eq!(hex(ChannelProfile::DirectMic, 0, false), "#b7445d");
        assert_eq!(hex(ChannelProfile::DirectMic, 0, true), "#f2798f");
        assert_eq!(hex(ChannelProfile::DirectMic, 1, false), "#b94642");
        assert_eq!(hex(ChannelProfile::RemoteParty, 0, false), "#6b61c4");
        assert_eq!(hex(ChannelProfile::RemoteParty, 1, true), "#bb8aef");
    }
}
