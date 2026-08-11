//! Generic, brand-agnostic macOS overlay primitives: non-activating panels,
//! Core Animation reveal/dismiss/fade mechanisms, appearance-aware color
//! helpers, main-thread dispatch, and interaction support for temporary
//! pasteboard contents and synthetic key chords. Consumers own app-specific
//! policy such as durations, key codes, animation keys, and brand colors. This
//! crate is macOS-only; on other platforms it exposes platform-neutral
//! geometry, layout, placement, and vector-path descriptions.

pub mod geometry;
pub mod layout;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod path;
pub mod placement;
