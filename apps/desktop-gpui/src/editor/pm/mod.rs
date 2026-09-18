//! A port of the ProseMirror model and replace algorithms the editor's
//! paste path runs — `DOMParser.parseSlice` over the note schema's
//! `parseDOM` rules, `Slice` / `ResolvedPos`, `Transform.replaceRange` with
//! its `Fitter`, and prosemirror-view's `parseFromClipboard` / `doPaste` —
//! so pasted HTML lands in the stored document exactly as it does in the
//! web view. Positions count characters where ProseMirror counts UTF-16
//! units; the structure of the result does not depend on the unit.

pub mod clipboard;
pub mod content;
pub mod dom;
pub mod node;
pub mod replace;
pub mod resolved;
pub mod schema;
pub mod serialize;
pub mod transform;

#[cfg(test)]
mod roundtrip;
