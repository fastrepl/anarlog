use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct GranolaResponse {
    pub docs: Vec<Document>,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<String>,
    pub notes: Option<ProseMirrorDoc>,
    pub notes_plain: Option<String>,
    pub last_viewed_panel: Option<LastViewedPanel>,
}

impl<'de> Deserialize<'de> for Document {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawDocument {
            id: String,
            title: String,
            #[serde(default)]
            content: String,
            created_at: String,
            updated_at: String,
            #[serde(default)]
            tags: Vec<String>,
            notes: Option<Value>,
            notes_plain: Option<String>,
            last_viewed_panel: Option<LastViewedPanel>,
        }

        let raw = RawDocument::deserialize(deserializer)?;

        let notes = raw.notes.and_then(|v| parse_maybe_stringified_json(&v));

        Ok(Document {
            id: raw.id,
            title: raw.title,
            content: raw.content,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            tags: raw.tags,
            notes,
            notes_plain: raw.notes_plain,
            last_viewed_panel: raw.last_viewed_panel,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LastViewedPanel {
    pub document_id: Option<String>,
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub title: Option<String>,
    pub content: Option<ProseMirrorDoc>,
    pub deleted_at: Option<String>,
    pub template_slug: Option<String>,
    pub last_viewed_at: Option<String>,
    pub updated_at: Option<String>,
    pub content_updated_at: Option<String>,
    pub original_content: String,
}

impl<'de> Deserialize<'de> for LastViewedPanel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawLastViewedPanel {
            document_id: Option<String>,
            id: Option<String>,
            created_at: Option<String>,
            title: Option<String>,
            content: Option<Value>,
            deleted_at: Option<String>,
            template_slug: Option<String>,
            last_viewed_at: Option<String>,
            updated_at: Option<String>,
            content_updated_at: Option<String>,
            #[serde(default)]
            original_content: String,
        }

        let raw = RawLastViewedPanel::deserialize(deserializer)?;

        let content = raw.content.and_then(|v| parse_maybe_stringified_json(&v));

        Ok(LastViewedPanel {
            document_id: raw.document_id,
            id: raw.id,
            created_at: raw.created_at,
            title: raw.title,
            content,
            deleted_at: raw.deleted_at,
            template_slug: raw.template_slug,
            last_viewed_at: raw.last_viewed_at,
            updated_at: raw.updated_at,
            content_updated_at: raw.content_updated_at,
            original_content: raw.original_content,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProseMirrorDoc {
    #[serde(rename = "type")]
    pub doc_type: String,
    #[serde(default)]
    pub content: Vec<ProseMirrorNode>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProseMirrorNode {
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub content: Vec<ProseMirrorNode>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub attrs: Option<serde_json::Map<String, Value>>,
}

fn parse_maybe_stringified_json(value: &Value) -> Option<ProseMirrorDoc> {
    match value {
        Value::Null => None,
        Value::Object(_) => serde_json::from_value(value.clone()).ok(),
        Value::String(s) => {
            let trimmed = s.trim_start();
            if trimmed.starts_with('<') {
                return None;
            }
            serde_json::from_str(s).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_document_with_notes_object() {
        let json = r#"{
            "id": "doc-1",
            "title": "Test",
            "content": "",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "tags": [],
            "notes": {"type": "doc", "content": []}
        }"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert!(doc.notes.is_some());
        assert_eq!(doc.notes.unwrap().doc_type, "doc");
    }

    #[test]
    fn test_parse_document_with_notes_string() {
        let json = r#"{
            "id": "doc-1",
            "title": "Test",
            "content": "",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "tags": [],
            "notes": "{\"type\": \"doc\", \"content\": []}"
        }"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert!(doc.notes.is_some());
        assert_eq!(doc.notes.unwrap().doc_type, "doc");
    }

    #[test]
    fn test_parse_document_with_html_content_skipped() {
        let json = r#"{
            "id": "doc-1",
            "title": "Test",
            "content": "",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "tags": [],
            "notes": "<html>content</html>"
        }"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert!(doc.notes.is_none());
    }
}
