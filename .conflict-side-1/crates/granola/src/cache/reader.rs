use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub id: String,
    pub document_id: String,
    pub start_timestamp: String,
    pub end_timestamp: String,
    pub text: String,
    pub source: String,
    pub is_final: bool,
}

#[derive(Debug, Clone)]
pub struct CacheDocument {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug)]
pub struct CacheData {
    pub documents: HashMap<String, CacheDocument>,
    pub transcripts: HashMap<String, Vec<TranscriptSegment>>,
}

#[derive(Deserialize)]
struct OuterCache {
    cache: String,
}

#[derive(Deserialize)]
struct InnerCache {
    state: CacheState,
}

#[derive(Deserialize)]
struct CacheState {
    documents: HashMap<String, Value>,
    transcripts: HashMap<String, Value>,
}

pub fn read_cache(path: &Path) -> Result<CacheData> {
    let content = std::fs::read_to_string(path).map_err(Error::CacheFileRead)?;

    let outer: OuterCache = serde_json::from_str(&content).map_err(Error::CacheJsonParse)?;

    let inner: InnerCache = serde_json::from_str(&outer.cache).map_err(Error::CacheJsonParse)?;

    let mut documents = HashMap::new();
    for (id, value) in inner.state.documents {
        if let Some(doc) = parse_cache_document(&id, &value) {
            documents.insert(id, doc);
        }
    }

    let mut transcripts = HashMap::new();
    for (id, value) in inner.state.transcripts {
        if let Some(segments) = parse_transcript_segments(&value) {
            transcripts.insert(id, segments);
        }
    }

    Ok(CacheData {
        documents,
        transcripts,
    })
}

fn parse_cache_document(id: &str, value: &Value) -> Option<CacheDocument> {
    #[derive(Deserialize)]
    struct RawDoc {
        title: Option<String>,
        created_at: Option<String>,
        updated_at: Option<String>,
    }

    let raw: RawDoc = serde_json::from_value(value.clone()).ok()?;

    Some(CacheDocument {
        id: id.to_string(),
        title: raw.title.unwrap_or_default(),
        created_at: raw.created_at.unwrap_or_default(),
        updated_at: raw.updated_at.unwrap_or_default(),
    })
}

fn parse_transcript_segments(value: &Value) -> Option<Vec<TranscriptSegment>> {
    #[derive(Deserialize)]
    struct RawSegment {
        id: String,
        document_id: String,
        start_timestamp: String,
        end_timestamp: String,
        text: String,
        source: String,
        is_final: bool,
    }

    let raw_segments: Vec<RawSegment> = serde_json::from_value(value.clone()).ok()?;

    Some(
        raw_segments
            .into_iter()
            .map(|s| TranscriptSegment {
                id: s.id,
                document_id: s.document_id,
                start_timestamp: s.start_timestamp,
                end_timestamp: s.end_timestamp,
                text: s.text,
                source: s.source,
                is_final: s.is_final,
            })
            .collect(),
    )
}

pub fn default_cache_path() -> std::path::PathBuf {
    if let Some(home) = dirs::home_dir() {
        #[cfg(target_os = "macos")]
        return home.join("Library/Application Support/Granola/cache-v3.json");

        #[cfg(target_os = "linux")]
        return home.join(".config/Granola/cache-v3.json");

        #[cfg(target_os = "windows")]
        return home.join("AppData/Roaming/Granola/cache-v3.json");

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return home.join("cache-v3.json");
    }
    std::path::PathBuf::from("cache-v3.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_cache_success() {
        let cache_content = r#"{"cache": "{\"state\":{\"documents\":{\"doc-1\":{\"title\":\"Test\",\"created_at\":\"2024-01-01T00:00:00Z\",\"updated_at\":\"2024-01-01T00:00:00Z\"}},\"transcripts\":{\"doc-1\":[{\"id\":\"seg-1\",\"document_id\":\"doc-1\",\"start_timestamp\":\"2024-01-01T14:00:00Z\",\"end_timestamp\":\"2024-01-01T14:00:05Z\",\"text\":\"Hello\",\"source\":\"system\",\"is_final\":true}]}}}"}"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(cache_content.as_bytes()).unwrap();

        let cache_data = read_cache(file.path()).unwrap();

        assert_eq!(cache_data.documents.len(), 1);
        assert_eq!(cache_data.transcripts.len(), 1);
        assert_eq!(cache_data.documents["doc-1"].title, "Test");
    }
}
