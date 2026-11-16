//! The open note's read-only Transcript view: shipping's segment list (speaker label in its
//! segment color, then the words), rendered by the same `anlg_transcript` segmenter.

use anlg_transcript::{
    ChannelProfile, RenderTranscriptRequest, RenderedTranscriptSegment, SegmentKey,
    render_transcript_segments,
};
use desktop_runtime::{RuntimeHandle, SessionId};
use gpui::{
    Context, FontWeight, Hsla, Render, SharedString, Task, Window, div, prelude::*, px, rgb,
};
use serde_json::{Value, json};

use crate::{meeting::model::Transcript, ui::theme::theme};

pub struct TranscriptView {
    segments: Vec<RenderedTranscriptSegment>,
    error: Option<String>,
    _watch: Task<()>,
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
        Self {
            segments: Vec::new(),
            error: None,
            _watch: watch,
        }
    }
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

impl Render for TranscriptView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let dark = colors.background.l < 0.5;
        div()
            .id("note-transcript")
            .size_full()
            .overflow_y_scroll()
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
            .children(self.segments.iter().enumerate().map(|(index, segment)| {
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
                                .text_color(segment_color(&segment.key, dark))
                                .child(SharedString::from(segment.speaker_label.clone())),
                        )
                        .child(
                            div()
                                .mt(px(6.))
                                .text_sm()
                                .line_height(px(22.75))
                                .text_color(colors.foreground)
                                .child(SharedString::from(segment.text.trim().to_owned())),
                        ),
                )
            }))
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
}
