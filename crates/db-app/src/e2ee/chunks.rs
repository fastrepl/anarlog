//! Large JSON array columns sync as fixed-size chunks. An append then re-seals
//! one small record instead of the whole array, edits to different parts of
//! the array merge instead of replacing each other, and no single record has
//! to carry a whole transcript.
//!
//! A column already using chunks never syncs as a plain field. Its
//! records are the virtual fields `column#0`, `column#1`, ... holding one
//! chunk each, plus `column#n` holding the chunk count. New columns keep the
//! whole-value format while 1.4.23 is supported: it rejects virtual fields.
//! Readers with unknown-record parking can defer them until they update.

use serde_json::{Value, json};

const CHUNKED_ARRAY_COLUMNS: &[(&str, &str, usize)] = &[("transcripts", "words_json", 256)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChunkPart {
    Count,
    Index(usize),
}

pub(super) fn chunk_size_for(table: &str, column: &str) -> Option<usize> {
    CHUNKED_ARRAY_COLUMNS
        .iter()
        .find(|(chunked_table, chunked_column, _)| {
            *chunked_table == table && *chunked_column == column
        })
        .map(|(_, _, size)| *size)
}

pub(super) fn chunk_field(column: &str, index: usize) -> String {
    format!("{column}#{index}")
}

pub(super) fn chunk_count_field(column: &str) -> String {
    format!("{column}#n")
}

/// Splits a virtual field name into its column and part. Only columns that
/// are chunked in `table` qualify, so a real column containing `#` is never
/// mistaken for one.
pub(super) fn parse_chunk_field<'a>(table: &str, field: &'a str) -> Option<(&'a str, ChunkPart)> {
    let (column, part) = field.rsplit_once('#')?;
    chunk_size_for(table, column)?;
    let part = if part == "n" {
        ChunkPart::Count
    } else {
        ChunkPart::Index(part.parse().ok()?)
    };
    Some((column, part))
}

/// The array stored in a chunked column, when the column holds one. Anything
/// else falls back to whole-value sync.
pub(super) fn parse_array(value: &Value) -> Option<Vec<Value>> {
    match serde_json::from_str::<Value>(value.as_str()?).ok()? {
        Value::Array(items) => Some(items),
        _ => None,
    }
}

pub(super) fn split_chunks(items: &[Value], chunk_size: usize) -> Vec<Vec<Value>> {
    if items.is_empty() {
        return Vec::new();
    }
    items
        .chunks(chunk_size.max(1))
        .map(<[Value]>::to_vec)
        .collect()
}

pub(super) fn join_chunks(chunks: &[Vec<Value>]) -> Value {
    let items = chunks.iter().flatten().cloned().collect::<Vec<_>>();
    Value::String(Value::Array(items).to_string())
}

pub(super) fn chunk_value(chunks: &[Vec<Value>], part: ChunkPart) -> Value {
    match part {
        ChunkPart::Count => json!(chunks.len()),
        ChunkPart::Index(index) => chunks
            .get(index)
            .map(|chunk| Value::Array(chunk.clone()))
            .unwrap_or(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_chunk_fields_only_for_chunked_columns() {
        assert_eq!(
            parse_chunk_field("transcripts", "words_json#12"),
            Some(("words_json", ChunkPart::Index(12)))
        );
        assert_eq!(
            parse_chunk_field("transcripts", "words_json#n"),
            Some(("words_json", ChunkPart::Count))
        );
        assert_eq!(parse_chunk_field("transcripts", "memo#1"), None);
        assert_eq!(parse_chunk_field("sessions", "words_json#1"), None);
        assert_eq!(parse_chunk_field("transcripts", "words_json#x"), None);
    }

    #[test]
    fn splits_and_joins_round_trip() {
        let items = (0..10)
            .map(|index| json!({ "id": index }))
            .collect::<Vec<_>>();
        let chunks = split_chunks(&items, 4);
        assert_eq!(
            chunks.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![4, 4, 2]
        );
        assert_eq!(chunk_value(&chunks, ChunkPart::Count), json!(3));
        assert_eq!(chunk_value(&chunks, ChunkPart::Index(5)), Value::Null);
        let joined = join_chunks(&chunks);
        assert_eq!(parse_array(&joined).unwrap(), items);
        assert!(split_chunks(&[], 4).is_empty());
    }
}
