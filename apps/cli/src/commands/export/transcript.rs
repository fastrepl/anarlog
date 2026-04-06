use std::path::Path;

use sqlx::SqlitePool;

use super::TranscriptFormat;
use super::helpers::build_segments;
use crate::error::CliResult;
use crate::{db, output};

pub(super) async fn transcript(
    pool: &SqlitePool,
    meeting_id: &str,
    format: TranscriptFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let words = db!(hypr_db_app::load_words(pool, meeting_id), "load words");
    let hints = db!(hypr_db_app::load_hints(pool, meeting_id), "load hints");

    let segments = build_segments(&words, &hints);

    match format {
        TranscriptFormat::Json => {
            let value: Vec<_> = segments
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "speaker": s.speaker,
                        "start_ms": s.start_ms,
                        "end_ms": s.end_ms,
                        "text": s.text,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        TranscriptFormat::Text => {
            let mut buf = String::new();
            for seg in &segments {
                let ts = output::format_timestamp_ms(seg.start_ms);
                buf.push_str(&format!("{} ({ts}): {}\n", seg.speaker, seg.text));
            }
            output::write_text(out, buf).await
        }
        TranscriptFormat::Srt => {
            let mut buf = String::new();
            for (i, seg) in segments.iter().enumerate() {
                buf.push_str(&format!("{}\n", i + 1));
                buf.push_str(&format!(
                    "{} --> {}\n",
                    subtitle_timestamp(seg.start_ms, ','),
                    subtitle_timestamp(seg.end_ms, ',')
                ));
                if seg.speaker != "Speaker" {
                    buf.push_str(&format!("{}: {}\n\n", seg.speaker, seg.text));
                } else {
                    buf.push_str(&format!("{}\n\n", seg.text));
                }
            }
            output::write_text(out, buf).await
        }
        TranscriptFormat::Vtt => {
            let mut buf = String::from("WEBVTT\n\n");
            for seg in &segments {
                buf.push_str(&format!(
                    "{} --> {}\n",
                    subtitle_timestamp(seg.start_ms, '.'),
                    subtitle_timestamp(seg.end_ms, '.')
                ));
                if seg.speaker != "Speaker" {
                    buf.push_str(&format!("{}: {}\n\n", seg.speaker, seg.text));
                } else {
                    buf.push_str(&format!("{}\n\n", seg.text));
                }
            }
            output::write_text(out, buf).await
        }
    }
}

fn subtitle_timestamp(ms: i64, sep: char) -> String {
    let ms = ms.max(0);
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let f = ms % 1_000;
    format!("{h:02}:{m:02}:{s:02}{sep}{f:03}")
}
