mod document;
mod error;

pub use document::Document;
pub use error::Error;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Meta {
        title: String,
        #[serde(default)]
        tags: Vec<String>,
    }

    mod parse {
        use super::*;
        use std::str::FromStr;

        #[test]
        fn basic() {
            let input = r#"---
title: Hello World
tags:
  - rust
  - serde
---

This is the content."#;

            let doc: Document<Meta> = Document::from_str(input).unwrap();
            assert_eq!(doc.frontmatter.title, "Hello World");
            assert_eq!(doc.frontmatter.tags, vec!["rust", "serde"]);
            assert_eq!(doc.content, "This is the content.");
        }

        #[test]
        fn empty_frontmatter() {
            let input = r#"---
---

Content here."#;

            let doc: Document<HashMap<String, String>> = Document::from_str(input).unwrap();
            assert!(doc.frontmatter.is_empty());
            assert_eq!(doc.content, "Content here.");
        }

        #[test]
        fn content_with_dashes() {
            let input = r#"---
title: Test
---

Some content with --- dashes in the middle.
And another --- line."#;

            let doc: Document<Meta> = Document::from_str(input).unwrap();
            assert_eq!(doc.frontmatter.title, "Test");
            assert!(doc.content.contains("--- dashes"));
        }

        #[test]
        fn leading_whitespace() {
            let input = "   ---\ntitle: Whitespace\n---\n\nContent";
            let doc: Document<Meta> = Document::from_str(input).unwrap();
            assert_eq!(doc.frontmatter.title, "Whitespace");
        }

        #[test]
        fn missing_opening_delimiter() {
            let input = "No frontmatter here";
            let result: Result<Document<Meta>, _> = Document::from_str(input);
            assert!(matches!(result, Err(Error::MissingOpeningDelimiter)));
        }

        #[test]
        fn missing_closing_delimiter() {
            let input = "---\ntitle: Test\nNo closing delimiter";
            let result: Result<Document<Meta>, _> = Document::from_str(input);
            assert!(matches!(result, Err(Error::MissingClosingDelimiter)));
        }

        #[test]
        fn single_newline_before_content() {
            let input = "---\ntitle: Test\n---\nContent";
            let doc: Document<Meta> = Document::from_str(input).unwrap();
            assert_eq!(doc.frontmatter.title, "Test");
            assert_eq!(doc.content, "Content");
        }

        #[test]
        fn empty_content() {
            let input = "---\ntitle: Test\n---";
            let doc: Document<Meta> = Document::from_str(input).unwrap();
            assert_eq!(doc.frontmatter.title, "Test");
            assert_eq!(doc.content, "");
        }

        #[test]
        fn content_starting_with_delimiter() {
            let input = "---\ntitle: Test\n---\n\n---starts with dashes";
            let doc: Document<Meta> = Document::from_str(input).unwrap();
            assert_eq!(doc.frontmatter.title, "Test");
            assert_eq!(doc.content, "---starts with dashes");
        }

        #[test]
        fn content_with_leading_newlines_preserved() {
            let input = "---\ntitle: Test\n---\n\n\n\nContent with leading newlines";
            let doc: Document<Meta> = Document::from_str(input).unwrap();
            assert_eq!(doc.frontmatter.title, "Test");
            assert_eq!(doc.content, "\n\nContent with leading newlines");
        }

        #[test]
        fn windows_line_endings() {
            let input =
                "---\r\ntitle: Hello World\r\ntags:\r\n  - rust\r\n---\r\n\r\nThis is the content.";
            let doc: Document<Meta> = Document::from_str(input).unwrap();
            assert_eq!(doc.frontmatter.title, "Hello World");
            assert_eq!(doc.frontmatter.tags, vec!["rust"]);
            assert_eq!(doc.content, "This is the content.");
        }

        #[test]
        fn windows_line_endings_empty_frontmatter() {
            let input = "---\r\n---\r\n\r\nContent here.";
            let doc: Document<HashMap<String, String>> = Document::from_str(input).unwrap();
            assert!(doc.frontmatter.is_empty());
            assert_eq!(doc.content, "Content here.");
        }

        #[test]
        fn mixed_line_endings() {
            let input = "---\r\ntitle: Test\n---\n\r\nContent";
            let doc: Document<Meta> = Document::from_str(input).unwrap();
            assert_eq!(doc.frontmatter.title, "Test");
            assert_eq!(doc.content, "Content");
        }
    }

    mod serialize {
        use super::*;

        #[test]
        fn basic() {
            let doc = Document::new(
                Meta {
                    title: "My Title".to_string(),
                    tags: vec!["tag1".to_string()],
                },
                "Content goes here.",
            );

            insta::assert_snapshot!(doc.render().unwrap(), @r"
            ---
            tags:
            - tag1
            title: My Title
            ---

            Content goes here.
            ");
        }

        #[test]
        fn keys_sorted_alphabetically() {
            let mut fm = HashMap::new();
            fm.insert("zebra".to_string(), "last".to_string());
            fm.insert("apple".to_string(), "first".to_string());
            fm.insert("mango".to_string(), "middle".to_string());

            let doc = Document::new(fm, "Content");

            insta::assert_snapshot!(doc.render().unwrap(), @r"
            ---
            apple: first
            mango: middle
            zebra: last
            ---

            Content
            ");
        }

        #[test]
        fn nested_keys_sorted() {
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            struct Nested {
                inner: HashMap<String, String>,
                name: String,
            }

            let mut inner = HashMap::new();
            inner.insert("z_key".to_string(), "z_value".to_string());
            inner.insert("a_key".to_string(), "a_value".to_string());

            let doc = Document::new(
                Nested {
                    inner,
                    name: "test".to_string(),
                },
                "Content",
            );

            insta::assert_snapshot!(doc.render().unwrap(), @r"
            ---
            inner:
              a_key: a_value
              z_key: z_value
            name: test
            ---

            Content
            ");
        }
    }

    mod roundtrip {
        use super::*;
        use std::str::FromStr;

        #[test]
        fn preserves_data() {
            let original = Document::new(
                Meta {
                    title: "Roundtrip Test".to_string(),
                    tags: vec!["a".to_string(), "b".to_string()],
                },
                "Some content.\n\nWith multiple paragraphs.",
            );

            let serialized = original.render().unwrap();
            let parsed: Document<Meta> = Document::from_str(&serialized).unwrap();

            assert_eq!(original.frontmatter, parsed.frontmatter);
            assert_eq!(original.content, parsed.content);
        }

        #[test]
        fn hashmap_frontmatter() {
            let mut fm = HashMap::new();
            fm.insert("key1".to_string(), "value1".to_string());
            fm.insert("key2".to_string(), "value2".to_string());

            let doc = Document::new(fm, "Content");
            let serialized = doc.render().unwrap();
            let parsed: Document<HashMap<String, String>> =
                Document::from_str(&serialized).unwrap();

            assert_eq!(parsed.frontmatter.get("key1"), Some(&"value1".to_string()));
            assert_eq!(parsed.frontmatter.get("key2"), Some(&"value2".to_string()));
        }

        #[test]
        fn content_with_leading_newlines() {
            let original = Document::new(
                Meta {
                    title: "Test".to_string(),
                    tags: vec![],
                },
                "\n\nContent after blank lines",
            );

            let serialized = original.render().unwrap();
            let parsed: Document<Meta> = Document::from_str(&serialized).unwrap();

            assert_eq!(parsed.content, original.content);
        }

        #[test]
        fn empty_content_roundtrip() {
            let original = Document::new(
                Meta {
                    title: "Test".to_string(),
                    tags: vec![],
                },
                "",
            );

            let serialized = original.render().unwrap();
            let parsed: Document<Meta> = Document::from_str(&serialized).unwrap();

            assert_eq!(parsed.content, "");
        }
    }

    mod serde_impl {
        use super::*;

        #[test]
        fn json_roundtrip() {
            let doc = Document::new(
                Meta {
                    title: "Serde Test".to_string(),
                    tags: vec![],
                },
                "Content",
            );

            let json = serde_json::to_string(&doc).unwrap();
            let parsed: Document<Meta> = serde_json::from_str(&json).unwrap();

            assert_eq!(doc.frontmatter.title, parsed.frontmatter.title);
            assert_eq!(doc.content, parsed.content);
        }
    }
}
