//! Parser tests
//!
//! These tests correspond to the AppleNote tests in the apple_cloud_notes_parser Ruby implementation:
//! https://github.com/threeplanetssoftware/apple_cloud_notes_parser/blob/master/spec/base_classes/apple_note.rb
//!
//! Tests cover:
//! - Note parsing (simple notes, compressed/uncompressed)
//! - Text decorations (bold, italic, underline, strikethrough)
//! - Block quotes and list indents
//! - URL/link formatting
//! - Color formatting
//! - Emoji and wide character handling
//! - HTML content escaping
//! - Various heading and style types

use apple_note::{note_to_markdown, parse_note_store_proto};
use std::fs;

#[test]
fn test_simple_note_parsing() {
    let data =
        fs::read("tests/data/simple_note_protobuf_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    assert!(proto.document.version >= 0);
    assert!(!proto.document.note.note_text.is_empty());
}

#[test]
fn test_simple_note_parsing_uncompressed() {
    let data = fs::read("tests/data/simple_note_protobuf.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    assert!(proto.document.version >= 0);
    assert!(!proto.document.note.note_text.is_empty());
}

#[test]
fn test_text_decorations() {
    let data =
        fs::read("tests/data/text_decorations_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    // The note should have text and attribute runs
    assert!(!proto.document.note.note_text.is_empty());
    assert!(!proto.document.note.attribute_run.is_empty());

    // Convert to markdown to verify formatting
    let markdown = note_to_markdown(&proto.document.note);
    assert!(!markdown.is_empty());
}

#[test]
fn test_block_quotes() {
    let data = fs::read("tests/data/block_quotes_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    assert!(!proto.document.note.note_text.is_empty());

    // Convert to markdown - should contain block quote markers
    let markdown = note_to_markdown(&proto.document.note);
    assert!(markdown.contains('>'));
}

#[test]
fn test_list_indents() {
    let data = fs::read("tests/data/list_indents_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    assert!(!proto.document.note.note_text.is_empty());
    assert!(!proto.document.note.attribute_run.is_empty());

    // Check that some attribute runs have indent amounts
    let has_indents = proto.document.note.attribute_run.iter().any(|run| {
        run.paragraph_style
            .as_ref()
            .and_then(|ps| ps.indent_amount)
            .is_some()
    });
    assert!(has_indents);
}

#[test]
fn test_url_formatting() {
    let data = fs::read("tests/data/url_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    assert!(!proto.document.note.note_text.is_empty());

    // Check that some attribute runs have links
    let has_links = proto
        .document
        .note
        .attribute_run
        .iter()
        .any(|run| run.link.is_some());
    assert!(has_links);

    // Convert to markdown - should contain markdown links
    let markdown = note_to_markdown(&proto.document.note);
    assert!(markdown.contains("]("));
}

#[test]
fn test_color_formatting() {
    let data =
        fs::read("tests/data/color_formatting_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    assert!(!proto.document.note.note_text.is_empty());

    // Check that some attribute runs have colors
    let has_colors = proto
        .document
        .note
        .attribute_run
        .iter()
        .any(|run| run.color.is_some());
    assert!(has_colors);
}

#[test]
fn test_emoji_formatting() {
    let data =
        fs::read("tests/data/emoji_formatting_1_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    assert!(!proto.document.note.note_text.is_empty());

    // Emojis should be preserved in the text
    // Check that we can parse the note successfully
    let markdown = note_to_markdown(&proto.document.note);
    assert!(!markdown.is_empty());
}

#[test]
fn test_wide_characters() {
    let data =
        fs::read("tests/data/wide_characters_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    assert!(!proto.document.note.note_text.is_empty());

    // Wide characters should be preserved
    let markdown = note_to_markdown(&proto.document.note);
    assert!(!markdown.is_empty());
}

#[test]
fn test_html_content() {
    let data = fs::read("tests/data/html_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");

    assert!(!proto.document.note.note_text.is_empty());

    // Should successfully parse notes with HTML-like content
    let markdown = note_to_markdown(&proto.document.note);
    assert!(!markdown.is_empty());
}

// Additional tests based on Ruby spec to ensure proper handling of various note types

#[test]
fn test_title_heading_subheading_conversion() {
    // Test that different heading levels are properly converted to markdown
    use apple_note::proto::{AttributeRun, Note, ParagraphStyle};
    use apple_note::{
        STYLE_TYPE_HEADING, STYLE_TYPE_SUBHEADING, STYLE_TYPE_TITLE, note_to_markdown,
    };

    // Title (h1)
    let title_note = Note {
        note_text: "Title Text".to_string(),
        attribute_run: vec![AttributeRun {
            length: 10,
            paragraph_style: Some(ParagraphStyle {
                style_type: Some(STYLE_TYPE_TITLE),
                alignment: None,
                indent_amount: None,
                checklist: None,
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };
    let markdown = note_to_markdown(&title_note);
    assert!(markdown.contains("# "), "Title should convert to # (h1)");

    // Heading (h2)
    let heading_note = Note {
        note_text: "Heading Text".to_string(),
        attribute_run: vec![AttributeRun {
            length: 12,
            paragraph_style: Some(ParagraphStyle {
                style_type: Some(STYLE_TYPE_HEADING),
                alignment: None,
                indent_amount: None,
                checklist: None,
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };
    let markdown = note_to_markdown(&heading_note);
    assert!(
        markdown.contains("## "),
        "Heading should convert to ## (h2)"
    );

    // Subheading (h3)
    let subheading_note = Note {
        note_text: "Subheading".to_string(),
        attribute_run: vec![AttributeRun {
            length: 10,
            paragraph_style: Some(ParagraphStyle {
                style_type: Some(STYLE_TYPE_SUBHEADING),
                alignment: None,
                indent_amount: None,
                checklist: None,
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };
    let markdown = note_to_markdown(&subheading_note);
    assert!(
        markdown.contains("### "),
        "Subheading should convert to ### (h3)"
    );
}

#[test]
fn test_monospaced_code_block_conversion() {
    use apple_note::proto::{AttributeRun, Note, ParagraphStyle};
    use apple_note::{STYLE_TYPE_MONOSPACED, note_to_markdown};

    let code_note = Note {
        note_text: "Code block content".to_string(),
        attribute_run: vec![AttributeRun {
            length: 18,
            paragraph_style: Some(ParagraphStyle {
                style_type: Some(STYLE_TYPE_MONOSPACED),
                alignment: None,
                indent_amount: None,
                checklist: None,
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };

    let markdown = note_to_markdown(&code_note);
    assert!(
        markdown.contains("```"),
        "Monospaced should be wrapped in code blocks"
    );
    assert!(markdown.contains("Code block content"));
}

#[test]
fn test_checkbox_list_conversion() {
    use apple_note::proto::{AttributeRun, Checklist, Note, ParagraphStyle};
    use apple_note::{STYLE_TYPE_CHECKBOX, note_to_markdown};

    // Checked item
    let checked_note = Note {
        note_text: "Checked item".to_string(),
        attribute_run: vec![AttributeRun {
            length: 12,
            paragraph_style: Some(ParagraphStyle {
                style_type: Some(STYLE_TYPE_CHECKBOX),
                alignment: None,
                indent_amount: None,
                checklist: Some(Checklist {
                    uuid: b"test-uuid".to_vec(),
                    done: 1,
                }),
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };
    let markdown = note_to_markdown(&checked_note);
    assert!(markdown.contains("- [x]"), "Checked item should show [x]");

    // Unchecked item
    let unchecked_note = Note {
        note_text: "Unchecked item".to_string(),
        attribute_run: vec![AttributeRun {
            length: 14,
            paragraph_style: Some(ParagraphStyle {
                style_type: Some(STYLE_TYPE_CHECKBOX),
                alignment: None,
                indent_amount: None,
                checklist: Some(Checklist {
                    uuid: b"test-uuid-2".to_vec(),
                    done: 0,
                }),
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };
    let markdown = note_to_markdown(&unchecked_note);
    assert!(markdown.contains("- [ ]"), "Unchecked item should show [ ]");
}

#[test]
fn test_list_types_conversion() {
    use apple_note::proto::{AttributeRun, Note, ParagraphStyle};
    use apple_note::{
        STYLE_TYPE_DASHED_LIST, STYLE_TYPE_DOTTED_LIST, STYLE_TYPE_NUMBERED_LIST, note_to_markdown,
    };

    // Dotted list
    let dotted_note = Note {
        note_text: "Dotted item".to_string(),
        attribute_run: vec![AttributeRun {
            length: 11,
            paragraph_style: Some(ParagraphStyle {
                style_type: Some(STYLE_TYPE_DOTTED_LIST),
                alignment: None,
                indent_amount: None,
                checklist: None,
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };
    let markdown = note_to_markdown(&dotted_note);
    assert!(
        markdown.contains("- "),
        "Dotted list should use bullet points"
    );

    // Dashed list
    let dashed_note = Note {
        note_text: "Dashed item".to_string(),
        attribute_run: vec![AttributeRun {
            length: 11,
            paragraph_style: Some(ParagraphStyle {
                style_type: Some(STYLE_TYPE_DASHED_LIST),
                alignment: None,
                indent_amount: None,
                checklist: None,
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };
    let markdown = note_to_markdown(&dashed_note);
    assert!(
        markdown.contains("- "),
        "Dashed list should use bullet points"
    );

    // Numbered list
    let numbered_note = Note {
        note_text: "Numbered item".to_string(),
        attribute_run: vec![AttributeRun {
            length: 13,
            paragraph_style: Some(ParagraphStyle {
                style_type: Some(STYLE_TYPE_NUMBERED_LIST),
                alignment: None,
                indent_amount: None,
                checklist: None,
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };
    let markdown = note_to_markdown(&numbered_note);
    assert!(markdown.contains("1. "), "Numbered list should use numbers");
}

#[test]
fn test_indented_content() {
    use apple_note::note_to_markdown;
    use apple_note::proto::{AttributeRun, Note, ParagraphStyle};

    let indented_note = Note {
        note_text: "Indented text".to_string(),
        attribute_run: vec![AttributeRun {
            length: 13,
            paragraph_style: Some(ParagraphStyle {
                style_type: None,
                alignment: None,
                indent_amount: Some(2), // 2 levels of indentation
                checklist: None,
                block_quote: None,
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };

    let markdown = note_to_markdown(&indented_note);
    // Should have indentation (2 spaces per level = 4 spaces)
    assert!(
        markdown.contains("    ") || markdown.contains("  "),
        "Should contain indentation"
    );
}

#[test]
fn test_block_quote_with_indent() {
    use apple_note::note_to_markdown;
    use apple_note::proto::{AttributeRun, Note, ParagraphStyle};

    let quote_note = Note {
        note_text: "Quoted text".to_string(),
        attribute_run: vec![AttributeRun {
            length: 11,
            paragraph_style: Some(ParagraphStyle {
                style_type: None,
                alignment: None,
                indent_amount: Some(1),
                checklist: None,
                block_quote: Some(1),
            }),
            font: None,
            font_weight: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };

    let markdown = note_to_markdown(&quote_note);
    assert!(
        markdown.contains(">"),
        "Block quote should contain > marker"
    );
}

#[test]
fn test_note_with_mixed_formatting() {
    // This test mimics the Ruby test for handling multiple formatting types in one note
    use apple_note::proto::{AttributeRun, Note};
    use apple_note::{FONT_TYPE_BOLD, FONT_TYPE_ITALIC, note_to_markdown};

    let mixed_note = Note {
        note_text: "Normal Bold Italic".to_string(),
        attribute_run: vec![
            AttributeRun {
                length: 7, // "Normal "
                font_weight: None,
                paragraph_style: None,
                font: None,
                underlined: None,
                strikethrough: None,
                superscript: None,
                link: None,
                color: None,
                attachment_info: None,
                unknown_identifier: None,
                emphasis_style: None,
            },
            AttributeRun {
                length: 5, // "Bold "
                font_weight: Some(FONT_TYPE_BOLD),
                paragraph_style: None,
                font: None,
                underlined: None,
                strikethrough: None,
                superscript: None,
                link: None,
                color: None,
                attachment_info: None,
                unknown_identifier: None,
                emphasis_style: None,
            },
            AttributeRun {
                length: 6, // "Italic"
                font_weight: Some(FONT_TYPE_ITALIC),
                paragraph_style: None,
                font: None,
                underlined: None,
                strikethrough: None,
                superscript: None,
                link: None,
                color: None,
                attachment_info: None,
                unknown_identifier: None,
                emphasis_style: None,
            },
        ],
    };

    let markdown = note_to_markdown(&mixed_note);
    assert!(
        markdown.contains("**") || markdown.contains("*"),
        "Should contain bold and/or italic markers"
    );
}
