pub mod accessibility;
mod app_link;
mod attachments;
mod clipboard;
pub mod document;
pub mod drop;
pub mod find;
pub mod holds;
mod html;
pub mod icons;
mod input;
pub mod menu;
pub mod model;
mod pane;
pub mod persistence;
mod sequence;
mod services;
pub(crate) mod surface;
mod tasks;
pub mod title;
pub mod transcript;
mod transform;

pub use attachments::{AttachmentPreview, AttachmentService};
pub use pane::EditorPane;

#[cfg(test)]
mod pane_tests;
#[cfg(test)]
mod tests;
