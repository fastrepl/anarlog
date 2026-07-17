//! Utility function tests
//!
//! These tests correspond to the helper tests in the apple_cloud_notes_parser Ruby implementation:
//! https://github.com/threeplanetssoftware/apple_cloud_notes_parser/blob/master/spec/base_classes/apple_note.rb
//!
//! Tests cover:
//! - Core time to Unix time conversion (convert_core_time in Ruby)
//! - GZIP detection (is_gzip in Ruby)
//! - GZIP decompression

use apple_note::utils::{core_time_to_unix, decompress_gzip, is_gzip};
use std::fs;

#[test]
fn test_core_time_conversion() {
    // iOS Core Time epoch is 2001-01-01 00:00:00 UTC
    // Unix epoch is 1970-01-01 00:00:00 UTC
    // The offset is 978307200 seconds

    // Test zero (2001-01-01 00:00:00)
    assert_eq!(core_time_to_unix(0), 978307200);

    // Test a known timestamp
    // 608413790 in Core Time should be 1586721790 in Unix time
    assert_eq!(core_time_to_unix(608413790), 1586720990);
}

#[test]
fn test_core_time_negative() {
    // Test negative values (before Core Time epoch)
    let negative_time = -1000;
    assert_eq!(core_time_to_unix(negative_time), 978307200 - 1000);
}

#[test]
fn test_core_time_large_value() {
    // Test large positive timestamp
    let large_time = 1_000_000_000_i64;
    assert_eq!(core_time_to_unix(large_time), 978307200 + 1_000_000_000);
}

#[test]
fn test_core_time_boundary_values() {
    // Test boundary values that don't overflow
    // i64::MIN would overflow when adding the offset, so we test a safe minimum
    let safe_min = i64::MIN + 978307200;
    assert_eq!(core_time_to_unix(safe_min - 978307200), safe_min);

    // Test a large positive value that doesn't overflow
    let large_value = i64::MAX - 978307200;
    assert_eq!(core_time_to_unix(large_value), i64::MAX);
}

#[test]
fn test_is_gzip_detection() {
    // Test with gzipped file
    let gzipped_data =
        fs::read("tests/data/simple_note_protobuf_gzipped.bin").expect("Failed to read test data");
    assert!(is_gzip(&gzipped_data));

    // Test with non-gzipped file
    let plain_data =
        fs::read("tests/data/simple_note_protobuf.bin").expect("Failed to read test data");
    assert!(!is_gzip(&plain_data));

    // Test with empty data
    assert!(!is_gzip(&[]));

    // Test with single byte
    assert!(!is_gzip(&[0x1f]));
}

#[test]
fn test_is_gzip_with_magic_bytes() {
    // Test with correct magic bytes (requires > 2 bytes)
    assert!(is_gzip(&[0x1f, 0x8b, 0x08]));
    assert!(is_gzip(&[0x1f, 0x8b, 0x00, 0x00]));

    // Test with incorrect magic bytes
    assert!(!is_gzip(&[0x1f, 0x8c, 0x08]));
    assert!(!is_gzip(&[0x1e, 0x8b, 0x08]));
    assert!(!is_gzip(&[0x00, 0x00, 0x00]));
}

#[test]
fn test_is_gzip_exact_length() {
    // Test with exactly 3 bytes (minimum for gzip detection, since impl requires > 2)
    assert!(is_gzip(&[0x1f, 0x8b, 0x00]));
    assert!(!is_gzip(&[0x1f, 0x00, 0x00]));

    // Test that 2 bytes is not enough
    assert!(!is_gzip(&[0x1f, 0x8b]));
}

#[test]
fn test_gzip_decompression() {
    let gzipped_data =
        fs::read("tests/data/simple_note_protobuf_gzipped.bin").expect("Failed to read test data");
    let plain_data =
        fs::read("tests/data/simple_note_protobuf.bin").expect("Failed to read test data");

    let decompressed = decompress_gzip(&gzipped_data).expect("Failed to decompress");

    // The decompressed data should match the plain data
    assert_eq!(decompressed, plain_data);
}

#[test]
fn test_gzip_decompression_error() {
    // Try to decompress non-gzipped data
    let plain_data =
        fs::read("tests/data/simple_note_protobuf.bin").expect("Failed to read test data");

    let result = decompress_gzip(&plain_data);
    assert!(result.is_err());
}

#[test]
fn test_gzip_decompression_empty() {
    // Try to decompress empty data
    let result = decompress_gzip(&[]);
    assert!(result.is_err());
}

#[test]
fn test_gzip_decompression_multiple_files() {
    // Test decompression with various gzipped test files
    let files = [
        "tests/data/simple_note_protobuf_gzipped.bin",
        "tests/data/table_gzipped.bin",
        "tests/data/table_formats_gzipped.bin",
        "tests/data/right_to_left_table_gzipped.bin",
    ];

    for file in &files {
        let data = fs::read(file).expect(&format!("Failed to read {}", file));
        assert!(is_gzip(&data), "{} should be detected as gzip", file);

        let decompressed = decompress_gzip(&data).expect(&format!("Failed to decompress {}", file));
        assert!(
            !decompressed.is_empty(),
            "{} should decompress to non-empty data",
            file
        );

        // Verify decompressed data is not gzipped
        assert!(
            !is_gzip(&decompressed),
            "Decompressed data from {} should not be gzipped",
            file
        );
    }
}

#[test]
fn test_gzip_roundtrip_check() {
    let gzipped_data =
        fs::read("tests/data/simple_note_protobuf_gzipped.bin").expect("Failed to read test data");

    // Decompress once
    let decompressed = decompress_gzip(&gzipped_data).expect("Failed to decompress");

    // The decompressed data should not be gzipped
    assert!(!is_gzip(&decompressed));

    // Trying to decompress again should fail
    let result = decompress_gzip(&decompressed);
    assert!(
        result.is_err(),
        "Decompressing already decompressed data should fail"
    );
}

#[test]
fn test_gzip_decompression_size() {
    let gzipped_data =
        fs::read("tests/data/simple_note_protobuf_gzipped.bin").expect("Failed to read test data");
    let plain_data =
        fs::read("tests/data/simple_note_protobuf.bin").expect("Failed to read test data");

    let decompressed = decompress_gzip(&gzipped_data).expect("Failed to decompress");

    // Verify the sizes match
    assert_eq!(
        decompressed.len(),
        plain_data.len(),
        "Decompressed size should match original"
    );
}

#[test]
fn test_core_time_consistency() {
    // Test that conversion is consistent
    let times = [0, 100, 1000, 10000, 608413790, 1000000000];
    for &time in &times {
        let unix = core_time_to_unix(time);
        assert_eq!(unix, time + 978307200);
    }
}
