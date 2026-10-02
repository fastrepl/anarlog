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
            for (input, content) in [
                (
                    r#"---
---

Content here."#,
                    "Content here.",
                ),
                ("---\r\n---\r\n\r\nContent here.", "Content here."),
            ] {
                let doc: Document<HashMap<String, String>> = Document::from_str(input).unwrap();
                assert!(doc.frontmatter.is_empty(), "{input:?}");
                assert_eq!(doc.content, content, "{input:?}");
            }
        }

        #[test]
        fn edge_cases() {
            for (input, title, tags, content) in [
                (
                    r#"---
title: Test
---

Some content with --- dashes in the middle.
And another --- line."#,
                    "Test",
                    &[][..],
                    "Some content with --- dashes in the middle.\nAnd another --- line.",
                ),
                (
                    "   ---\ntitle: Whitespace\n---\n\nContent",
                    "Whitespace",
                    &[],
                    "Content",
                ),
                ("---\ntitle: Test\n---\nContent", "Test", &[], "Content"),
                ("---\ntitle: Test\n---", "Test", &[], ""),
                (
                    "---\ntitle: Test\n---\n\n---starts with dashes",
                    "Test",
                    &[],
                    "---starts with dashes",
                ),
                (
                    "---\ntitle: Test\n---\n\n\n\nContent with leading newlines",
                    "Test",
                    &[],
                    "\n\nContent with leading newlines",
                ),
                (
                    "---\r\ntitle: Test\n---\n\r\nContent",
                    "Test",
                    &[],
                    "Content",
                ),
                (
                    "---\r\ntitle: Hello World\r\ntags:\r\n  - rust\r\n---\r\n\r\nThis is the content.",
                    "Hello World",
                    &["rust"],
                    "This is the content.",
                ),
            ] {
                let doc: Document<Meta> = Document::from_str(input).unwrap();
                assert_eq!(doc.frontmatter.title, title, "{input:?}");
                assert_eq!(
                    doc.frontmatter.tags,
                    tags.iter().map(|tag| tag.to_string()).collect::<Vec<_>>(),
                    "{input:?}"
                );
                assert_eq!(doc.content, content, "{input:?}");
            }
        }

        #[test]
        fn rejects_missing_delimiters() {
            for (input, expect_opening) in [
                ("No frontmatter here", true),
                ("---\ntitle: Test\nNo closing delimiter", false),
            ] {
                let result: Result<Document<Meta>, _> = Document::from_str(input);
                let matched = match &result {
                    Err(Error::MissingOpeningDelimiter) => expect_opening,
                    Err(Error::MissingClosingDelimiter) => !expect_opening,
                    _ => false,
                };
                assert!(matched, "unexpected parse result for {input:?}");
            }
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
        fn preserves_frontmatter_and_content() {
            for content in [
                "Some content.\n\nWith multiple paragraphs.",
                "\n\nContent after blank lines",
                "",
            ] {
                let original = Document::new(
                    Meta {
                        title: "Roundtrip Test".to_string(),
                        tags: vec!["a".to_string(), "b".to_string()],
                    },
                    content,
                );

                let serialized = original.render().unwrap();
                let parsed: Document<Meta> = Document::from_str(&serialized).unwrap();

                assert_eq!(original.frontmatter, parsed.frontmatter, "{content:?}");
                assert_eq!(original.content, parsed.content, "{content:?}");
            }
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
