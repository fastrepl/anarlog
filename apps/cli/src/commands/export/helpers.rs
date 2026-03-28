use std::path::Path;

use crate::error::CliResult;
use crate::output;

pub(super) struct Segment {
    pub speaker: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

pub(super) struct TableColumn {
    pub key: &'static str,
    pub header: &'static str,
}

pub(super) struct TableCell {
    pub value: serde_json::Value,
    pub text: String,
}

impl TableCell {
    pub fn new(value: serde_json::Value, text: impl Into<String>) -> Self {
        Self {
            value,
            text: text.into(),
        }
    }
}

pub(super) fn build_segments(
    words: &[hypr_transcript::FinalizedWord],
    hints: &[hypr_db_app::StorageSpeakerHint],
) -> Vec<Segment> {
    use hypr_db_app::StorageSpeakerHintData;

    let hint_map: std::collections::HashMap<&str, &hypr_db_app::StorageSpeakerHint> =
        hints.iter().map(|h| (h.word_id.as_str(), h)).collect();

    let mut segments: Vec<Segment> = Vec::new();

    for word in words {
        let speaker = hint_map
            .get(word.id.as_str())
            .map(|h| match &h.data {
                StorageSpeakerHintData::UserSpeakerAssignment { human_id } => human_id.clone(),
                StorageSpeakerHintData::ProviderSpeakerIndex {
                    speaker_index,
                    channel,
                    ..
                } => {
                    let ch = channel.unwrap_or(word.channel);
                    format!("Speaker {ch}-{speaker_index}")
                }
            })
            .unwrap_or_else(|| format!("Channel {}", word.channel));

        if let Some(last) = segments.last_mut()
            && last.speaker == speaker
        {
            last.text.push(' ');
            last.text.push_str(&word.text);
            last.end_ms = word.end_ms;
            continue;
        }

        segments.push(Segment {
            speaker,
            start_ms: word.start_ms,
            end_ms: word.end_ms,
            text: word.text.clone(),
        });
    }

    segments
}

pub(super) fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub(super) fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}

pub(super) async fn write_table(
    format: super::TableFormat,
    out: Option<&Path>,
    columns: &[TableColumn],
    rows: &[Vec<TableCell>],
) -> CliResult<()> {
    match format {
        super::TableFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|row| {
                    columns
                        .iter()
                        .zip(row.iter())
                        .map(|(column, cell)| (column.key.to_string(), cell.value.clone()))
                        .collect::<serde_json::Map<_, _>>()
                })
                .collect();
            output::write_json(out, &value).await
        }
        super::TableFormat::Csv => {
            let mut buf = String::new();
            buf.push_str(
                &columns
                    .iter()
                    .map(|column| column.header)
                    .collect::<Vec<_>>()
                    .join(","),
            );
            buf.push('\n');

            for row in rows {
                buf.push_str(
                    &row.iter()
                        .map(|cell| csv_escape(&cell.text))
                        .collect::<Vec<_>>()
                        .join(","),
                );
                buf.push('\n');
            }

            output::write_text(out, buf).await
        }
        super::TableFormat::Text => {
            let mut buf = String::new();
            for row in rows {
                buf.push_str(
                    &row.iter()
                        .map(|cell| cell.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\t"),
                );
                buf.push('\n');
            }

            output::write_text(out, buf).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_table_json_keeps_values_by_column_key() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("rows.json");

        write_table(
            super::super::TableFormat::Json,
            Some(&out),
            &[TableColumn {
                key: "name",
                header: "name",
            }],
            &[vec![TableCell::new(serde_json::Value::Null, "")]],
        )
        .await
        .unwrap();

        let rendered = std::fs::read_to_string(out).unwrap();
        assert!(rendered.contains("\"name\": null"));
    }
}
