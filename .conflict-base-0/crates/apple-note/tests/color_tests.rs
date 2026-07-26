//! Color tests
//!
//! These tests correspond to the Color tests in the apple_cloud_notes_parser Ruby implementation:
//! https://github.com/threeplanetssoftware/apple_cloud_notes_parser/blob/master/spec/base_classes/proto_patches.rb
//!
//! Tests cover hex string generation for RGB color values.

use apple_note::Color;

#[test]
fn test_red_hex_string_100_percent() {
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
fn test_hex_string_0_percent_with_padding() {
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
fn test_blue_hex_string_100_percent() {
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
fn test_green_hex_string_100_percent() {
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

#[test]
fn test_white_color() {
    let color = Color {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };
    assert_eq!(color.full_hex_string(), "#FFFFFF");
}

#[test]
fn test_partial_color_values() {
    let color = Color {
        red: 0.5,
        green: 0.25,
        blue: 0.75,
        alpha: 0.0,
    };
    assert_eq!(color.red_hex_string(), "7F");
    assert_eq!(color.green_hex_string(), "3F");
    assert_eq!(color.blue_hex_string(), "BF");
}

#[test]
fn test_color_equality() {
    let color1 = Color {
        red: 1.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    };
    let color2 = Color {
        red: 1.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    };
    assert_eq!(color1, color2);
}

#[test]
fn test_color_inequality() {
    let color1 = Color {
        red: 1.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    };
    let color2 = Color {
        red: 0.0,
        green: 1.0,
        blue: 0.0,
        alpha: 0.0,
    };
    assert_ne!(color1, color2);
}
