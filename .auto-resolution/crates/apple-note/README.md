# apple-note

A Rust library for parsing Apple Notes protobuf data, extracting text content, and handling embedded objects like tables.

## Credits

This crate is a Rust port of [apple_cloud_notes_parser](https://github.com/threeplanetssoftware/apple_cloud_notes_parser) by [Three Planets Software](https://github.com/threeplanetssoftware). The original Ruby implementation provides comprehensive parsing of Apple Notes data from iOS and macOS backups.

The test suite in this crate follows the structure and test cases from the original Ruby implementation's spec directory to ensure compatibility and correctness.

## Features

- Parse Apple Notes protobuf data (gzipped and uncompressed)
- Extract plaintext and formatted text spans from notes
- Handle various text formatting (bold, italic, underline, strikethrough, links, colors)
- Parse embedded tables with support for RTL languages
- Detect and classify embedded object types (images, drawings, PDFs, audio, video, etc.)
- Convert notes to Markdown format

## Usage

```rust
use apple_note::{parse_note_store_proto, note_to_markdown, extract_plaintext};

// Parse a note from gzipped protobuf data
let data = std::fs::read("note_data.bin")?;
let proto = parse_note_store_proto(&data)?;

// Extract plaintext
let text = extract_plaintext(&proto.document.note);

// Convert to markdown
let markdown = note_to_markdown(&proto.document.note);
```

## Test Data

The test data files in `tests/data/` are derived from the test fixtures in the original apple_cloud_notes_parser repository.

## License

See the repository root for license information.
