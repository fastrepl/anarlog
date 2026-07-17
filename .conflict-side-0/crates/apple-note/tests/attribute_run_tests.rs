//! AttributeRun tests
//!
//! These tests correspond to the AttributeRun tests in the apple_cloud_notes_parser Ruby implementation:
//! https://github.com/threeplanetssoftware/apple_cloud_notes_parser/blob/master/spec/base_classes/proto_patches.rb
//!
//! Tests cover style comparison between attribute runs, including font weight, underlined,
//! strikethrough, superscript, link, paragraph style, font, and color comparisons.

use apple_note::{AttributeRun, Color, Font, ParagraphStyle};

#[test]
fn test_same_style_no_styles() {
    let run1 = AttributeRun {
        length: 1,
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
    };
    let run2 = AttributeRun {
        length: 1,
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
    };
    assert!(run1.same_style(&run2));
}

#[test]
fn test_different_font_weights() {
    let run1 = AttributeRun {
        length: 1,
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
    };
    let run2 = AttributeRun {
        length: 1,
        font_weight: Some(4),
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
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_different_underlined_values() {
    let run1 = AttributeRun {
        length: 1,
        underlined: Some(2),
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
    };
    let run2 = AttributeRun {
        length: 1,
        underlined: Some(3),
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
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_different_strikethrough_values() {
    let run1 = AttributeRun {
        length: 1,
        strikethrough: Some(2),
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
    };
    let run2 = AttributeRun {
        length: 1,
        strikethrough: Some(3),
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
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_different_superscript_values() {
    let run1 = AttributeRun {
        length: 1,
        superscript: Some(2),
        font_weight: None,
        paragraph_style: None,
        font: None,
        underlined: None,
        strikethrough: None,
        link: None,
        color: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    let run2 = AttributeRun {
        length: 1,
        superscript: Some(3),
        font_weight: None,
        paragraph_style: None,
        font: None,
        underlined: None,
        strikethrough: None,
        link: None,
        color: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_different_link_values() {
    let run1 = AttributeRun {
        length: 1,
        link: Some("https://google.com".to_string()),
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
    };
    let run2 = AttributeRun {
        length: 1,
        link: Some("http://yahoo.com".to_string()),
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
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_same_paragraph_styles() {
    let style1 = ParagraphStyle {
        style_type: None,
        alignment: None,
        indent_amount: Some(1),
        checklist: None,
        block_quote: None,
    };
    let style2 = ParagraphStyle {
        style_type: None,
        alignment: None,
        indent_amount: Some(1),
        checklist: None,
        block_quote: None,
    };
    let run1 = AttributeRun {
        length: 1,
        paragraph_style: Some(style1),
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
    };
    let run2 = AttributeRun {
        length: 1,
        paragraph_style: Some(style2),
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
    };
    assert!(run1.same_style(&run2));
}

#[test]
fn test_different_paragraph_styles() {
    let style1 = ParagraphStyle {
        style_type: None,
        alignment: None,
        indent_amount: Some(1),
        checklist: None,
        block_quote: None,
    };
    let style2 = ParagraphStyle {
        style_type: None,
        alignment: None,
        indent_amount: Some(2),
        checklist: None,
        block_quote: None,
    };
    let run1 = AttributeRun {
        length: 1,
        paragraph_style: Some(style1),
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
    };
    let run2 = AttributeRun {
        length: 1,
        paragraph_style: Some(style2),
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
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_same_fonts() {
    let font1 = Font {
        font_name: Some("Consolas".to_string()),
        point_size: None,
        font_hints: None,
    };
    let font2 = Font {
        font_name: Some("Consolas".to_string()),
        point_size: None,
        font_hints: None,
    };
    let run1 = AttributeRun {
        length: 1,
        font: Some(font1),
        paragraph_style: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        color: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    let run2 = AttributeRun {
        length: 1,
        font: Some(font2),
        paragraph_style: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        color: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    assert!(run1.same_style(&run2));
}

#[test]
fn test_different_fonts() {
    let font1 = Font {
        font_name: Some("Consolas".to_string()),
        point_size: None,
        font_hints: None,
    };
    let font2 = Font {
        font_name: Some("Times New Roman".to_string()),
        point_size: None,
        font_hints: None,
    };
    let run1 = AttributeRun {
        length: 1,
        font: Some(font1),
        paragraph_style: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        color: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    let run2 = AttributeRun {
        length: 1,
        font: Some(font2),
        paragraph_style: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        color: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_same_colors() {
    let color1 = Color {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };
    let color2 = Color {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };
    let run1 = AttributeRun {
        length: 1,
        color: Some(color1),
        paragraph_style: None,
        font: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    let run2 = AttributeRun {
        length: 1,
        color: Some(color2),
        paragraph_style: None,
        font: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    assert!(run1.same_style(&run2));
}

#[test]
fn test_different_colors() {
    let color1 = Color {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };
    let color2 = Color {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    };
    let run1 = AttributeRun {
        length: 1,
        color: Some(color1),
        paragraph_style: None,
        font: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    let run2 = AttributeRun {
        length: 1,
        color: Some(color2),
        paragraph_style: None,
        font: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_mixed_attributes_same_style() {
    let color = Color {
        red: 1.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    };
    let font = Font {
        font_name: Some("Arial".to_string()),
        point_size: Some(12.0),
        font_hints: None,
    };
    let run1 = AttributeRun {
        length: 5,
        font_weight: Some(1),
        underlined: Some(1),
        color: Some(color.clone()),
        font: Some(font.clone()),
        paragraph_style: None,
        strikethrough: None,
        superscript: None,
        link: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    let run2 = AttributeRun {
        length: 5,
        font_weight: Some(1),
        underlined: Some(1),
        color: Some(color),
        font: Some(font),
        paragraph_style: None,
        strikethrough: None,
        superscript: None,
        link: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    assert!(run1.same_style(&run2));
}

#[test]
fn test_mixed_attributes_different_style() {
    let color1 = Color {
        red: 1.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    };
    let color2 = Color {
        red: 0.0,
        green: 1.0,
        blue: 0.0,
        alpha: 1.0,
    };
    let font = Font {
        font_name: Some("Arial".to_string()),
        point_size: Some(12.0),
        font_hints: None,
    };
    let run1 = AttributeRun {
        length: 5,
        font_weight: Some(1),
        underlined: Some(1),
        color: Some(color1),
        font: Some(font.clone()),
        paragraph_style: None,
        strikethrough: None,
        superscript: None,
        link: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    let run2 = AttributeRun {
        length: 5,
        font_weight: Some(1),
        underlined: Some(1),
        color: Some(color2),
        font: Some(font),
        paragraph_style: None,
        strikethrough: None,
        superscript: None,
        link: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_none_vs_some_link() {
    let run1 = AttributeRun {
        length: 1,
        link: None,
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
    };
    let run2 = AttributeRun {
        length: 1,
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
    };
    assert!(!run1.same_style(&run2));
}

#[test]
fn test_same_link() {
    let run1 = AttributeRun {
        length: 1,
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
    };
    let run2 = AttributeRun {
        length: 1,
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
    };
    assert!(run1.same_style(&run2));
}

#[test]
fn test_clone_attribute_run() {
    let run = AttributeRun {
        length: 10,
        font_weight: Some(1),
        underlined: Some(1),
        paragraph_style: None,
        font: None,
        strikethrough: None,
        superscript: None,
        link: Some("https://test.com".to_string()),
        color: None,
        attachment_info: None,
        unknown_identifier: None,
        emphasis_style: None,
    };
    let cloned = run.clone();
    assert_eq!(run.length, cloned.length);
    assert_eq!(run.font_weight, cloned.font_weight);
    assert_eq!(run.underlined, cloned.underlined);
    assert_eq!(run.link, cloned.link);
    assert!(run.same_style(&cloned));
}
