use crate::api::Document;
use crate::cache::{CacheDocument, TranscriptSegment};
use crate::error::{Error, Result};
use crate::markdown::document_to_markdown;
use crate::transcript::format_transcript;
use chrono::{DateTime, Utc};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn write_notes(documents: &[Document], output_dir: &Path) -> Result<usize> {
    fs::create_dir_all(output_dir).map_err(Error::CreateDirectory)?;

    let mut used_filenames: HashMap<String, usize> = HashMap::new();
    let mut written_count = 0;

    let invalid_chars = Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).unwrap();
    let multiple_underscores = Regex::new(r"_+").unwrap();

    for doc in documents {
        let filename =
            sanitize_filename(&doc.title, &doc.id, &invalid_chars, &multiple_underscores);
        let filename = make_unique(&filename, &mut used_filenames);
        *used_filenames.entry(filename.clone()).or_insert(0) += 1;

        let file_path = output_dir.join(format!("{}.md", filename));

        if !should_update_file(&doc.updated_at, &file_path) {
            continue;
        }

        let markdown = document_to_markdown(doc)?;

        fs::write(&file_path, markdown).map_err(|e| Error::WriteFile {
            path: file_path.display().to_string(),
            source: e,
        })?;

        written_count += 1;
    }

    Ok(written_count)
}

pub fn write_transcripts(
    documents: &HashMap<String, CacheDocument>,
    transcripts: &HashMap<String, Vec<TranscriptSegment>>,
    output_dir: &Path,
) -> Result<usize> {
    fs::create_dir_all(output_dir).map_err(Error::CreateDirectory)?;

    let mut used_filenames: HashMap<String, usize> = HashMap::new();
    let mut written_count = 0;

    let invalid_chars = Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).unwrap();
    let multiple_underscores = Regex::new(r"_+").unwrap();

    for (doc_id, segments) in transcripts {
        if segments.is_empty() {
            continue;
        }

        let doc = documents
            .get(doc_id)
            .cloned()
            .unwrap_or_else(|| CacheDocument {
                id: doc_id.clone(),
                title: doc_id.clone(),
                created_at: String::new(),
                updated_at: String::new(),
            });

        let filename =
            sanitize_filename(&doc.title, &doc.id, &invalid_chars, &multiple_underscores);
        let filename = make_unique(&filename, &mut used_filenames);
        *used_filenames.entry(filename.clone()).or_insert(0) += 1;

        let file_path = output_dir.join(format!("{}.txt", filename));

        if !should_update_file(&doc.updated_at, &file_path) {
            continue;
        }

        let content = format_transcript(&doc, segments);
        if content.is_empty() {
            continue;
        }

        fs::write(&file_path, content).map_err(|e| Error::WriteFile {
            path: file_path.display().to_string(),
            source: e,
        })?;

        written_count += 1;
    }

    Ok(written_count)
}

fn sanitize_filename(
    title: &str,
    id: &str,
    invalid_chars: &Regex,
    multiple_underscores: &Regex,
) -> String {
    let name = if title.trim().is_empty() {
        id
    } else {
        title.trim()
    };

    let name = invalid_chars.replace_all(name, "_");
    let name = multiple_underscores.replace_all(&name, "_");
    let name = name.trim_matches('_');

    let name = if name.is_empty() { "untitled" } else { name };

    if name.chars().count() > 100 {
        name.chars().take(100).collect()
    } else {
        name.to_string()
    }
}

fn make_unique(filename: &str, used: &mut HashMap<String, usize>) -> String {
    if let Some(&count) = used.get(filename) {
        format!("{}_{}", filename, count + 1)
    } else {
        filename.to_string()
    }
}

fn should_update_file(updated_at: &str, file_path: &Path) -> bool {
    let metadata = match fs::metadata(file_path) {
        Ok(m) => m,
        Err(_) => return true,
    };

    let doc_updated = match DateTime::parse_from_rfc3339(updated_at) {
        Ok(dt) => dt,
        Err(_) => return true,
    };

    let file_modified = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return true,
    };

    let file_modified: DateTime<Utc> = file_modified.into();

    doc_updated > file_modified
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sanitize_filename() {
        let invalid_chars = Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).unwrap();
        let multiple_underscores = Regex::new(r"_+").unwrap();

        assert_eq!(
            sanitize_filename(
                "Simple Title",
                "id-1",
                &invalid_chars,
                &multiple_underscores
            ),
            "Simple Title"
        );
        assert_eq!(
            sanitize_filename(
                "Title: With Colon",
                "id-2",
                &invalid_chars,
                &multiple_underscores
            ),
            "Title_ With Colon"
        );
        assert_eq!(
            sanitize_filename(
                "Title/With/Slashes",
                "id-3",
                &invalid_chars,
                &multiple_underscores
            ),
            "Title_With_Slashes"
        );
        assert_eq!(
            sanitize_filename("", "id-4", &invalid_chars, &multiple_underscores),
            "id-4"
        );
        assert_eq!(
            sanitize_filename("   ", "id-5", &invalid_chars, &multiple_underscores),
            "id-5"
        );
    }

    #[test]
    fn test_make_unique() {
        let mut used = HashMap::new();

        assert_eq!(make_unique("test", &mut used), "test");
        used.insert("test".to_string(), 1);

        assert_eq!(make_unique("test", &mut used), "test_2");
    }

    #[test]
    fn test_write_notes() {
        let temp_dir = TempDir::new().unwrap();

        let documents = vec![Document {
            id: "doc-1".to_string(),
            title: "Test Meeting".to_string(),
            content: "Content".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            tags: vec![],
            notes: None,
            notes_plain: None,
            last_viewed_panel: None,
        }];

        let count = write_notes(&documents, temp_dir.path()).unwrap();
        assert_eq!(count, 1);

        let file_path = temp_dir.path().join("Test Meeting.md");
        assert!(file_path.exists());

        let content = fs::read_to_string(file_path).unwrap();
        assert!(content.contains("# Test Meeting"));
    }
}
