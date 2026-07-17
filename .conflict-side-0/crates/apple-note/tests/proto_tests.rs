//! Proto type tests
//!
//! These tests correspond to the proto_patches tests in the apple_cloud_notes_parser Ruby implementation:
//! https://github.com/threeplanetssoftware/apple_cloud_notes_parser/blob/master/spec/base_classes/proto_patches.rb
//!
//! Tests cover Color hex string generation and AttributeRun style comparison.
//! Note: Some tests in this file overlap with color_tests.rs and attribute_run_tests.rs
//! for additional coverage of the proto module types.

use apple_note::proto::{AttributeRun, Color, Font, ParagraphStyle};

#[cfg(test)]
mod color_tests {
    use super::*;

    #[test]
    fn test_red_hex_string() {
        let color = Color {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        };
        assert_eq!(color.red_hex_string(), "FF");
        assert_eq!(color.green_hex_string(), "00");
        assert_eq!(color.blue_hex_string(), "00");
        assert_eq!(color.full_hex_string(), "#FF0000");
    }

    #[test]
    fn test_zero_hex_string() {
        let color = Color {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        };
        assert_eq!(color.red_hex_string(), "00");
        assert_eq!(color.green_hex_string(), "00");
        assert_eq!(color.blue_hex_string(), "00");
        assert_eq!(color.full_hex_string(), "#000000");
    }

    #[test]
    fn test_blue_hex_string() {
        let color = Color {
            red: 0.0,
            green: 0.0,
            blue: 1.0,
            alpha: 0.0,
        };
        assert_eq!(color.red_hex_string(), "00");
        assert_eq!(color.green_hex_string(), "00");
        assert_eq!(color.blue_hex_string(), "FF");
        assert_eq!(color.full_hex_string(), "#0000FF");
    }

    #[test]
    fn test_green_hex_string() {
        let color = Color {
            red: 0.0,
            green: 1.0,
            blue: 0.0,
            alpha: 0.0,
        };
        assert_eq!(color.red_hex_string(), "00");
        assert_eq!(color.green_hex_string(), "FF");
        assert_eq!(color.blue_hex_string(), "00");
        assert_eq!(color.full_hex_string(), "#00FF00");
    }
}

#[cfg(test)]
mod attribute_run_tests {
    use super::*;

    #[test]
    fn test_same_style_no_style() {
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
    fn test_different_font_weight() {
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
    fn test_different_underlined() {
        let run1 = AttributeRun {
            length: 1,
            underlined: Some(2),
            paragraph_style: None,
            font: None,
            font_weight: None,
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
            paragraph_style: None,
            font: None,
            font_weight: None,
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
    fn test_different_strikethrough() {
        let run1 = AttributeRun {
            length: 1,
            strikethrough: Some(2),
            paragraph_style: None,
            font: None,
            font_weight: None,
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
            paragraph_style: None,
            font: None,
            font_weight: None,
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
    fn test_different_superscript() {
        let run1 = AttributeRun {
            length: 1,
            superscript: Some(2),
            paragraph_style: None,
            font: None,
            font_weight: None,
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
            paragraph_style: None,
            font: None,
            font_weight: None,
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
    fn test_different_link() {
        let run1 = AttributeRun {
            length: 1,
            link: Some("https://google.com".to_string()),
            paragraph_style: None,
            font: None,
            font_weight: None,
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
            paragraph_style: None,
            font: None,
            font_weight: None,
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
    fn test_same_paragraph_style() {
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
    fn test_different_paragraph_style() {
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
    fn test_same_font() {
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
    fn test_different_font() {
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
    fn test_same_color() {
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
    fn test_different_color() {
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
}
