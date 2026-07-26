use std::{collections::HashMap, str::FromStr};

use hypr_frontmatter::{Document, Error as FrontmatterError};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ParsedDocument {
    pub frontmatter: HashMap<String, serde_json::Value>,
    pub content: String,
}

fn yaml_to_json(yaml: serde_yaml::Value) -> serde_json::Value {
    serde_json::to_value(&yaml).unwrap_or(serde_json::Value::Null)
}

impl FromStr for ParsedDocument {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Document::<HashMap<String, serde_yaml::Value>>::from_str(s) {
            Ok(doc) => {
                let frontmatter: HashMap<String, serde_json::Value> = doc
                    .frontmatter
                    .into_iter()
                    .map(|(k, v)| (k, yaml_to_json(v)))
                    .collect();

                Ok(ParsedDocument {
                    frontmatter,
                    content: doc.content,
                })
            }
            Err(FrontmatterError::MissingOpeningDelimiter) => Ok(ParsedDocument {
                frontmatter: HashMap::new(),
                content: s.to_string(),
            }),
            Err(e) => Err(e.into()),
        }
    }
}

impl ParsedDocument {
    pub fn render(&self) -> Result<String, crate::Error> {
        if self.frontmatter.is_empty() {
            return Ok(self.content.clone());
        }

        let frontmatter_yaml: HashMap<String, serde_yaml::Value> = self
            .frontmatter
            .iter()
            .map(|(k, v)| {
                let yaml_value = serde_yaml::to_value(v).unwrap_or(serde_yaml::Value::Null);
                (k.clone(), yaml_value)
            })
            .collect();

        let doc = Document::new(frontmatter_yaml, &self.content);
        doc.render().map_err(crate::Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::md_with_frontmatter;

    #[test]
    fn parse_without_frontmatter_returns_empty() {
        let input = "# Meeting Summary\n\nPlain markdown.";
        let result = ParsedDocument::from_str(input).unwrap();

        assert!(result.frontmatter.is_empty());
        assert_eq!(result.content, input);
    }

    #[test]
    fn parse_with_frontmatter() {
        let input = &md_with_frontmatter("id: test-id\ntype: memo", "Content here.");
        let result = ParsedDocument::from_str(input).unwrap();

        assert_eq!(result.frontmatter["id"], "test-id");
        assert_eq!(result.frontmatter["type"], "memo");
        assert_eq!(result.content, "Content here.");
    }

    #[test]
    fn render_roundtrip() {
        let input = &md_with_frontmatter("id: test-id\ntype: memo", "Content here.");
        let parsed = ParsedDocument::from_str(input).unwrap();
        let rendered = parsed.render().unwrap();
        let reparsed = ParsedDocument::from_str(&rendered).unwrap();

        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn parse_tags_array() {
        let input = &md_with_frontmatter("tags:\n  - meeting\n  - project-x\n  - important", "");
        let result = ParsedDocument::from_str(input).unwrap();

        let tags = result.frontmatter["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], "meeting");
        assert_eq!(tags[1], "project-x");
        assert_eq!(tags[2], "important");
    }

    #[test]
    fn parse_inline_tags_array() {
        let input = &md_with_frontmatter("tags: [daily, work]", "");
        let result = ParsedDocument::from_str(input).unwrap();

        let tags = result.frontmatter["tags"].as_array().unwrap();
        assert_eq!(tags, &vec!["daily", "work"]);
    }

    #[test]
    fn parse_aliases() {
        let input = &md_with_frontmatter("aliases:\n  - \"Weekly Sync\"\n  - \"Team Meeting\"", "");
        let result = ParsedDocument::from_str(input).unwrap();

        let aliases = result.frontmatter["aliases"].as_array().unwrap();
        assert_eq!(aliases[0], "Weekly Sync");
        assert_eq!(aliases[1], "Team Meeting");
    }

    #[test]
    fn parse_dates() {
        let input = &md_with_frontmatter("date: 2024-01-15\ncreated: 2024-01-15T10:30:00", "");
        let result = ParsedDocument::from_str(input).unwrap();

        assert_eq!(result.frontmatter["date"], "2024-01-15");
        assert_eq!(result.frontmatter["created"], "2024-01-15T10:30:00");
    }

    #[test]
    fn parse_boolean_values() {
        let input = &md_with_frontmatter("publish: true\ndraft: false", "");
        let result = ParsedDocument::from_str(input).unwrap();

        assert_eq!(result.frontmatter["publish"], true);
        assert_eq!(result.frontmatter["draft"], false);
    }

    #[test]
    fn parse_numeric_values() {
        let input = &md_with_frontmatter("priority: 1\nrating: 4.5", "");
        let result = ParsedDocument::from_str(input).unwrap();

        assert_eq!(result.frontmatter["priority"], 1);
        assert_eq!(result.frontmatter["rating"], 4.5);
    }

    #[test]
    fn parse_null_value() {
        let input = &md_with_frontmatter("description: null\ntitle: Test", "");
        let result = ParsedDocument::from_str(input).unwrap();

        assert!(result.frontmatter["description"].is_null());
        assert_eq!(result.frontmatter["title"], "Test");
    }

    #[test]
    fn parse_nested_object() {
        let input = &md_with_frontmatter("metadata:\n  author: John\n  version: 2", "");
        let result = ParsedDocument::from_str(input).unwrap();

        let metadata = result.frontmatter["metadata"].as_object().unwrap();
        assert_eq!(metadata["author"], "John");
        assert_eq!(metadata["version"], 2);
    }

    #[test]
    fn parse_complex_obsidian_note() {
        let frontmatter = r#"id: meeting-2024-01-15
title: "Q1 Planning Session"
date: 2024-01-15
tags:
  - meeting
  - quarterly
  - planning
aliases:
  - "Q1 Planning"
participants:
  - Alice
  - Bob
status: completed
publish: false"#;
        let input = &md_with_frontmatter(frontmatter, "# Meeting Notes\n\nDiscussed roadmap.");
        let result = ParsedDocument::from_str(input).unwrap();

        assert_eq!(result.frontmatter["id"], "meeting-2024-01-15");
        assert_eq!(result.frontmatter["title"], "Q1 Planning Session");
        assert_eq!(result.frontmatter["status"], "completed");
        assert_eq!(result.frontmatter["publish"], false);
        assert_eq!(result.frontmatter["tags"].as_array().unwrap().len(), 3);
        assert_eq!(
            result.frontmatter["participants"].as_array().unwrap().len(),
            2
        );
        assert_eq!(result.content, "# Meeting Notes\n\nDiscussed roadmap.");
    }
}
