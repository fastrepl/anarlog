pub mod accessibility;
mod attachments;
mod clipboard;
pub mod document;
mod html;
pub mod icons;
mod input;
pub mod menu;
pub mod model;
mod pane;
pub mod persistence;
mod sequence;
mod services;
mod surface;
pub mod title;
pub mod transcript;
mod transform;

pub use attachments::{AttachmentPreview, AttachmentService};
pub use pane::EditorPane;

#[cfg(test)]
mod tests;
