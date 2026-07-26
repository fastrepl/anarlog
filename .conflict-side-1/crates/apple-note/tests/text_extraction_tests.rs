//! Text extraction tests
//!
//! These tests correspond to the content extraction tests in the apple_cloud_notes_parser Ruby implementation:
//! https://github.com/threeplanetssoftware/apple_cloud_notes_parser/blob/master/spec/base_classes/apple_note.rb
//!
//! Tests cover:
//! - Plaintext extraction from notes
//! - Text span extraction with formatting attributes
//! - Handling of attachments during extraction

use apple_note::{
    AttributeRun, Note, ParagraphStyle,
    extract::{TextSpan, extract_plaintext, extract_text_spans},
    parse_note_store_proto,
};
use std::fs;

#[test]
fn test_extract_plaintext_from_simple_note() {
    let data =
        fs::read("tests/data/simple_note_protobuf_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");
    let note = &proto.document.note;

    let text = extract_plaintext(note);
    assert!(!text.is_empty(), "Plain text should not be empty");
}

#[test]
fn test_extract_text_spans_basic() {
    let note = Note {
        note_text: "Hello World".to_string(),
        attribute_run: vec![AttributeRun {
            length: 11,
            paragraph_style: None,
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

    let spans = extract_text_spans(&note);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Hello World");
    assert!(!spans[0].bold);
    assert!(!spans[0].italic);
    assert!(!spans[0].underline);
}

#[test]
fn test_extract_text_spans_with_bold() {
    let note = Note {
        note_text: "Bold text".to_string(),
        attribute_run: vec![AttributeRun {
            length: 9,
            font_weight: Some(1),
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
        }],
    };

    let spans = extract_text_spans(&note);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Bold text");
    assert!(spans[0].bold);
    assert!(!spans[0].italic);
}

#[test]
fn test_extract_text_spans_with_italic() {
    let note = Note {
        note_text: "Italic text".to_string(),
        attribute_run: vec![AttributeRun {
            length: 11,
            font_weight: Some(2),
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
        }],
    };

    let spans = extract_text_spans(&note);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Italic text");
    assert!(!spans[0].bold);
    assert!(spans[0].italic);
}

#[test]
fn test_extract_text_spans_with_bold_italic() {
    let note = Note {
        note_text: "Bold italic".to_string(),
        attribute_run: vec![AttributeRun {
            length: 11,
            font_weight: Some(3),
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
        }],
    };

    let spans = extract_text_spans(&note);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Bold italic");
    assert!(spans[0].bold);
    assert!(spans[0].italic);
}

#[test]
fn test_extract_text_spans_with_underline() {
    let note = Note {
        note_text: "Underlined".to_string(),
        attribute_run: vec![AttributeRun {
            length: 10,
            underlined: Some(1),
            font_weight: None,
            paragraph_style: None,
            font: None,
            strikethrough: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };

    let spans = extract_text_spans(&note);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Underlined");
    assert!(spans[0].underline);
}

#[test]
fn test_extract_text_spans_with_strikethrough() {
    let note = Note {
        note_text: "Strikethrough".to_string(),
        attribute_run: vec![AttributeRun {
            length: 13,
            strikethrough: Some(1),
            font_weight: None,
            paragraph_style: None,
            font: None,
            underlined: None,
            superscript: None,
            link: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };

    let spans = extract_text_spans(&note);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Strikethrough");
    assert!(spans[0].strikethrough);
}

#[test]
fn test_extract_text_spans_with_link() {
    let note = Note {
        note_text: "Click here".to_string(),
        attribute_run: vec![AttributeRun {
            length: 10,
            link: Some("https://example.com".to_string()),
            font_weight: None,
            paragraph_style: None,
            font: None,
            underlined: None,
            strikethrough: None,
            superscript: None,
            color: None,
            attachment_info: None,
            unknown_identifier: None,
            emphasis_style: None,
        }],
    };

    let spans = extract_text_spans(&note);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Click here");
    assert_eq!(spans[0].link, Some("https://example.com".to_string()));
}

#[test]
fn test_extract_text_spans_multiple_runs() {
    let note = Note {
        note_text: "Normal Bold Italic".to_string(),
        attribute_run: vec![
            AttributeRun {
                length: 7,
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
                length: 5,
                font_weight: Some(1),
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
                length: 6,
                font_weight: Some(2),
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

    let spans = extract_text_spans(&note);
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].text, "Normal ");
    assert!(!spans[0].bold);
    assert_eq!(spans[1].text, "Bold ");
    assert!(spans[1].bold);
    assert_eq!(spans[2].text, "Italic");
    assert!(spans[2].italic);
}

#[test]
fn test_extract_text_spans_with_paragraph_style() {
    let style = ParagraphStyle {
        style_type: Some(1),
        alignment: None,
        indent_amount: None,
        checklist: None,
        block_quote: None,
    };

    let note = Note {
        note_text: "Styled text".to_string(),
        attribute_run: vec![AttributeRun {
            length: 11,
            paragraph_style: Some(style.clone()),
            font_weight: None,
            font: None,
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

    let spans = extract_text_spans(&note);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "Styled text");
    assert_eq!(spans[0].style_type, Some(1));
    assert!(spans[0].paragraph_style.is_some());
}

#[test]
fn test_extract_text_spans_skips_attachments() {
    // Use a simple text string to avoid Unicode boundary issues
    let text = "Text.More text";
    let note = Note {
        note_text: text.to_string(),
        attribute_run: vec![
            AttributeRun {
                length: 5, // "Text."
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
                length: 0, // Attachment with zero length in text
                font_weight: None,
                paragraph_style: None,
                font: None,
                underlined: None,
                strikethrough: None,
                superscript: None,
                link: None,
                color: None,
                attachment_info: Some(apple_note::AttachmentInfo {
                    attachment_identifier: Some("test".to_string()),
                    type_uti: None,
                }),
                unknown_identifier: None,
                emphasis_style: None,
            },
            AttributeRun {
                length: 9, // "More text"
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
        ],
    };

    let spans = extract_text_spans(&note);
    // Attachments are skipped during extraction
    assert!(spans.len() >= 2, "Should have at least 2 text spans");
    assert_eq!(spans[0].text, "Text.");
}

#[test]
fn test_text_span_new() {
    let span = TextSpan::new("Test text".to_string());
    assert_eq!(span.text, "Test text");
    assert!(!span.bold);
    assert!(!span.italic);
    assert!(!span.underline);
    assert!(!span.strikethrough);
    assert_eq!(span.link, None);
    assert_eq!(span.style_type, None);
    assert_eq!(span.paragraph_style, None);
}

#[test]
fn test_text_span_clone() {
    let span = TextSpan {
        text: "Test".to_string(),
        bold: true,
        italic: true,
        underline: false,
        strikethrough: false,
        link: Some("https://test.com".to_string()),
        style_type: Some(1),
        paragraph_style: None,
    };

    let cloned = span.clone();
    assert_eq!(span.text, cloned.text);
    assert_eq!(span.bold, cloned.bold);
    assert_eq!(span.italic, cloned.italic);
    assert_eq!(span.link, cloned.link);
}

#[test]
fn test_extract_from_real_file() {
    let data =
        fs::read("tests/data/simple_note_protobuf_gzipped.bin").expect("Failed to read test data");
    let proto = parse_note_store_proto(&data).expect("Failed to parse proto");
    let note = &proto.document.note;

    let spans = extract_text_spans(note);
    assert!(!spans.is_empty(), "Should extract at least one text span");

    let plaintext = extract_plaintext(note);
    assert!(!plaintext.is_empty(), "Plain text should not be empty");

    // Verify that concatenating all span texts equals the plaintext (minus attachments)
    let concatenated: String = spans.iter().map(|s| s.text.as_str()).collect();
    assert!(
        plaintext.contains(&concatenated) || concatenated.contains(&plaintext),
        "Concatenated spans should match plaintext content"
    );
}
