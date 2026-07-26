use crate::cache::{CacheDocument, TranscriptSegment};
use chrono::DateTime;

pub fn format_transcript(doc: &CacheDocument, segments: &[TranscriptSegment]) -> String {
    if segments.is_empty() {
        return String::new();
    }

    let mut output = String::new();

    output.push_str(&"=".repeat(80));
    output.push('\n');

    if !doc.title.is_empty() {
        output.push_str(&doc.title);
        output.push('\n');
    }

    output.push_str(&format!("ID: {}\n", doc.id));

    if !doc.created_at.is_empty() {
        output.push_str(&format!("Created: {}\n", doc.created_at));
    }

    if !doc.updated_at.is_empty() {
        output.push_str(&format!("Updated: {}\n", doc.updated_at));
    }

    output.push_str(&format!("Segments: {}\n", segments.len()));

    output.push_str(&"=".repeat(80));
    output.push_str("\n\n");

    for segment in segments {
        let time = parse_timestamp(&segment.start_timestamp);
        let speaker = match segment.source.as_str() {
            "microphone" => "You",
            _ => "System",
        };

        output.push_str(&format!("[{}] {}: {}\n", time, speaker, segment.text));
    }

    output
}

fn parse_timestamp(timestamp: &str) -> String {
    DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|_| timestamp.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_transcript() {
        let doc = CacheDocument {
            id: "doc-1".to_string(),
            title: "Test Meeting".to_string(),
            created_at: "2024-01-01T14:00:00Z".to_string(),
            updated_at: "2024-01-01T15:00:00Z".to_string(),
        };

        let segments = vec![
            TranscriptSegment {
                id: "seg-1".to_string(),
                document_id: "doc-1".to_string(),
                start_timestamp: "2024-01-01T14:00:04Z".to_string(),
                end_timestamp: "2024-01-01T14:00:06Z".to_string(),
                text: "Hello everyone".to_string(),
                source: "system".to_string(),
                is_final: true,
            },
            TranscriptSegment {
                id: "seg-2".to_string(),
                document_id: "doc-1".to_string(),
                start_timestamp: "2024-01-01T14:00:06Z".to_string(),
                end_timestamp: "2024-01-01T14:00:08Z".to_string(),
                text: "Hi there".to_string(),
                source: "microphone".to_string(),
                is_final: true,
            },
        ];

        let result = format_transcript(&doc, &segments);

        assert!(result.contains("Test Meeting"));
        assert!(result.contains("ID: doc-1"));
        assert!(result.contains("Segments: 2"));
        assert!(result.contains("[14:00:04] System: Hello everyone"));
        assert!(result.contains("[14:00:06] You: Hi there"));
    }

    #[test]
    fn test_parse_timestamp() {
        assert_eq!(parse_timestamp("2024-01-01T14:30:45Z"), "14:30:45");
        assert_eq!(parse_timestamp("invalid"), "invalid");
    }
}
