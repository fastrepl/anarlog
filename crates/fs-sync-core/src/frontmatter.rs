use std::{collections::HashMap, str::FromStr};

use anlg_frontmatter::{Document, Error as FrontmatterError};

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
    fn parse_converts_yaml_values_to_json() {
        let frontmatter = r#"tags:
  - meeting
  - project-x
  - important
inline_tags: [daily, work]
aliases:
  - "Weekly Sync"
  - "Team Meeting"
date: 2024-01-15
created: 2024-01-15T10:30:00
publish: true
draft: false
priority: 1
rating: 4.5
description: null
title: Test
metadata:
  author: John
  version: 2"#;
        let input = &md_with_frontmatter(frontmatter, "Content here.");
        let result = ParsedDocument::from_str(input).unwrap();

        let tags = result.frontmatter["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], "meeting");
        assert_eq!(tags[1], "project-x");
        assert_eq!(tags[2], "important");

        let inline_tags = result.frontmatter["inline_tags"].as_array().unwrap();
        assert_eq!(inline_tags, &vec!["daily", "work"]);

        let aliases = result.frontmatter["aliases"].as_array().unwrap();
        assert_eq!(aliases[0], "Weekly Sync");
        assert_eq!(aliases[1], "Team Meeting");

        assert_eq!(result.frontmatter["date"], "2024-01-15");
        assert_eq!(result.frontmatter["created"], "2024-01-15T10:30:00");

        assert_eq!(result.frontmatter["publish"], true);
        assert_eq!(result.frontmatter["draft"], false);

        assert_eq!(result.frontmatter["priority"], 1);
        assert_eq!(result.frontmatter["rating"], 4.5);

        assert!(result.frontmatter["description"].is_null());
        assert_eq!(result.frontmatter["title"], "Test");

        let metadata = result.frontmatter["metadata"].as_object().unwrap();
        assert_eq!(metadata["author"], "John");
        assert_eq!(metadata["version"], 2);

        assert_eq!(result.content, "Content here.");
    }
}
