use crate::api::Document;
use crate::error::Result;
use crate::prosemirror::convert_to_markdown;
use serde::Serialize;

#[derive(Serialize)]
struct Metadata {
    id: String,
    created: String,
    updated: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
}

pub fn document_to_markdown(doc: &Document) -> Result<String> {
    let metadata = Metadata {
        id: doc.id.clone(),
        created: doc.created_at.clone(),
        updated: doc.updated_at.clone(),
        tags: doc.tags.clone(),
    };

    let yaml = serde_yaml::to_string(&metadata)?;

    let mut output = String::new();
    output.push_str("---\n");
    output.push_str(&yaml);
    output.push_str("---\n\n");

    if !doc.title.is_empty() {
        output.push_str(&format!("# {}\n\n", doc.title));
    }

    let content = get_document_content(doc);
    if !content.is_empty() {
        output.push_str(&content);
        if !content.ends_with('\n') {
            output.push('\n');
        }
    }

    Ok(output)
}

fn get_document_content(doc: &Document) -> String {
    if let Some(ref notes) = doc.notes {
        let content = convert_to_markdown(notes).trim().to_string();
        if !content.is_empty() {
            return content;
        }
    }

    if let Some(ref panel) = doc.last_viewed_panel {
        if let Some(ref content) = panel.content {
            let md = convert_to_markdown(content).trim().to_string();
            if !md.is_empty() {
                return md;
            }
        }

        if !panel.original_content.is_empty() {
            return panel.original_content.clone();
        }
    }

    doc.content.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_to_markdown() {
        let doc = Document {
            id: "test-123".to_string(),
            title: "Test Meeting".to_string(),
            content: "Meeting content".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
            tags: vec!["work".to_string(), "planning".to_string()],
            notes: None,
            notes_plain: None,
            last_viewed_panel: None,
        };

        let result = document_to_markdown(&doc).unwrap();

        assert!(result.contains("---"));
        assert!(result.contains("id: test-123"));
        assert!(result.contains("# Test Meeting"));
        assert!(result.contains("Meeting content"));
    }
}
