//! Embedded object tests
//!
//! These tests correspond to the embedded object and UTI tests in the apple_cloud_notes_parser Ruby implementation:
//! - https://github.com/threeplanetssoftware/apple_cloud_notes_parser/blob/master/spec/utilities/apple_uniform_type_identifier.rb
//! - https://github.com/threeplanetssoftware/apple_cloud_notes_parser/blob/master/spec/embedded_objects/embedded_objects.rb
//!
//! Tests cover UTI type detection, embedded object extraction, and type classification.

use apple_note::{EmbeddedObjectType, extract_embedded_objects};

#[test]
fn test_embedded_object_type_detection() {
    // Test various UTI type mappings
    assert_eq!(
        EmbeddedObjectType::from_uti("com.apple.notes.table"),
        EmbeddedObjectType::Table
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("com.apple.notes.ICTable"),
        EmbeddedObjectType::Table
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("public.image"),
        EmbeddedObjectType::Image
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("com.apple.drawing.2"),
        EmbeddedObjectType::Drawing
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("public.url"),
        EmbeddedObjectType::URL
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("public.audio"),
        EmbeddedObjectType::Audio
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("public.movie"),
        EmbeddedObjectType::Video
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("com.adobe.pdf"),
        EmbeddedObjectType::PDF
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("unknown.type"),
        EmbeddedObjectType::Unknown
    );
}

#[test]
fn test_embedded_object_type_detection_partial_match() {
    // Test partial matching for image, video, audio, pdf
    assert_eq!(
        EmbeddedObjectType::from_uti("public.jpeg.image"),
        EmbeddedObjectType::Image
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("public.mpeg4.video"),
        EmbeddedObjectType::Video
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("public.mp3.audio"),
        EmbeddedObjectType::Audio
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("public.pdf.document"),
        EmbeddedObjectType::PDF
    );
}

#[test]
fn test_extract_embedded_objects_from_note() {
    // Create a simple note with attachment info in attribute runs
    // We'll use a real test file if available, otherwise just test the logic
    use apple_note::proto::{AttachmentInfo, AttributeRun, Note};

    let mut note = Note {
        note_text: "Test text with attachment\u{FFFC}".to_string(),
        attribute_run: vec![],
    };

    // Add an attribute run without attachment
    note.attribute_run.push(AttributeRun {
        length: 25,
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
    });

    // Add an attribute run with a table attachment
    note.attribute_run.push(AttributeRun {
        length: 1,
        paragraph_style: None,
        font: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        color: None,
        attachment_info: Some(AttachmentInfo {
            attachment_identifier: Some("test-uuid-123".to_string()),
            type_uti: Some("com.apple.notes.table".to_string()),
        }),
        unknown_identifier: None,
        emphasis_style: None,
    });

    let objects = extract_embedded_objects(&note);

    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].object_type, EmbeddedObjectType::Table);
    assert_eq!(objects[0].uuid, "test-uuid-123");
    assert_eq!(objects[0].type_uti, "com.apple.notes.table");
}

#[test]
fn test_extract_multiple_embedded_objects() {
    use apple_note::proto::{AttachmentInfo, AttributeRun, Note};

    let mut note = Note {
        note_text: "Text\u{FFFC}more text\u{FFFC}".to_string(),
        attribute_run: vec![],
    };

    // Add text run
    note.attribute_run.push(AttributeRun {
        length: 4,
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
    });

    // Add table attachment
    note.attribute_run.push(AttributeRun {
        length: 1,
        paragraph_style: None,
        font: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        color: None,
        attachment_info: Some(AttachmentInfo {
            attachment_identifier: Some("table-uuid".to_string()),
            type_uti: Some("com.apple.notes.table".to_string()),
        }),
        unknown_identifier: None,
        emphasis_style: None,
    });

    // Add more text
    note.attribute_run.push(AttributeRun {
        length: 9,
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
    });

    // Add image attachment
    note.attribute_run.push(AttributeRun {
        length: 1,
        paragraph_style: None,
        font: None,
        font_weight: None,
        underlined: None,
        strikethrough: None,
        superscript: None,
        link: None,
        color: None,
        attachment_info: Some(AttachmentInfo {
            attachment_identifier: Some("image-uuid".to_string()),
            type_uti: Some("public.image".to_string()),
        }),
        unknown_identifier: None,
        emphasis_style: None,
    });

    let objects = extract_embedded_objects(&note);

    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0].object_type, EmbeddedObjectType::Table);
    assert_eq!(objects[0].uuid, "table-uuid");
    assert_eq!(objects[1].object_type, EmbeddedObjectType::Image);
    assert_eq!(objects[1].uuid, "image-uuid");
}

// Additional tests based on Ruby apple_uniform_type_identifier.rb spec

#[test]
fn test_uti_refuses_non_string() {
    // In Rust, this is enforced by the type system, so we test with empty/invalid strings
    // instead of non-String types
    let uti_type = EmbeddedObjectType::from_uti("");
    assert_eq!(uti_type, EmbeddedObjectType::Unknown);
}

#[test]
fn test_uti_identifies_unknown_type() {
    let uti_type = EmbeddedObjectType::from_uti("thisisamadeuputi");
    assert_eq!(uti_type, EmbeddedObjectType::Unknown);
}

#[test]
fn test_uti_recognizes_public_types() {
    // Ruby test: recognizes 'public' UTIs
    assert_eq!(
        EmbeddedObjectType::from_uti("public.thisisamadeuputi"),
        EmbeddedObjectType::Unknown
    );

    // Known public types should be recognized correctly
    assert_eq!(
        EmbeddedObjectType::from_uti("public.image"),
        EmbeddedObjectType::Image
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("public.audio"),
        EmbeddedObjectType::Audio
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("public.movie"),
        EmbeddedObjectType::Video
    );
}

#[test]
fn test_uti_recognizes_dynamic_types() {
    // Ruby test: recognizes dynamic UTIs (dyn.* prefix)
    // Dynamic UTIs should fall into Unknown category
    let uti_type = EmbeddedObjectType::from_uti("dyn.thisisamadeuputi");
    assert_eq!(uti_type, EmbeddedObjectType::Unknown);

    let uti_type2 = EmbeddedObjectType::from_uti("dyn.aghsjdgsa");
    assert_eq!(uti_type2, EmbeddedObjectType::Unknown);
}

#[test]
fn test_uti_handles_apple_specific_types() {
    assert_eq!(
        EmbeddedObjectType::from_uti("com.apple.notes.table"),
        EmbeddedObjectType::Table
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("com.apple.drawing.2"),
        EmbeddedObjectType::Drawing
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("com.apple.paper"),
        EmbeddedObjectType::Document
    );
    assert_eq!(
        EmbeddedObjectType::from_uti("com.apple.notes.gallery"),
        EmbeddedObjectType::Gallery
    );
}

#[test]
fn test_uti_handles_third_party_types() {
    assert_eq!(
        EmbeddedObjectType::from_uti("com.adobe.pdf"),
        EmbeddedObjectType::PDF
    );
}

#[test]
fn test_embedded_object_new() {
    let obj = apple_note::EmbeddedObject::new(
        EmbeddedObjectType::Table,
        "test-uuid".to_string(),
        "com.apple.notes.table".to_string(),
    );

    assert_eq!(obj.object_type, EmbeddedObjectType::Table);
    assert_eq!(obj.uuid, "test-uuid");
    assert_eq!(obj.type_uti, "com.apple.notes.table");
}

#[test]
fn test_embedded_object_clone() {
    let obj = apple_note::EmbeddedObject::new(
        EmbeddedObjectType::Image,
        "image-123".to_string(),
        "public.image".to_string(),
    );

    let cloned = obj.clone();
    assert_eq!(obj, cloned);
    assert_eq!(obj.uuid, cloned.uuid);
    assert_eq!(obj.type_uti, cloned.type_uti);
}

#[test]
fn test_embedded_object_equality() {
    let obj1 = apple_note::EmbeddedObject::new(
        EmbeddedObjectType::Table,
        "uuid-1".to_string(),
        "com.apple.notes.table".to_string(),
    );

    let obj2 = apple_note::EmbeddedObject::new(
        EmbeddedObjectType::Table,
        "uuid-1".to_string(),
        "com.apple.notes.table".to_string(),
    );

    assert_eq!(obj1, obj2);
}

#[test]
fn test_embedded_object_inequality() {
    let obj1 = apple_note::EmbeddedObject::new(
        EmbeddedObjectType::Table,
        "uuid-1".to_string(),
        "com.apple.notes.table".to_string(),
    );

    let obj2 = apple_note::EmbeddedObject::new(
        EmbeddedObjectType::Image,
        "uuid-2".to_string(),
        "public.image".to_string(),
    );

    assert_ne!(obj1, obj2);
}
