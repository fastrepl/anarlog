pub mod assets;
pub mod automation_runner;
pub mod automations;
pub mod calendar;
mod catalog;
mod catalog_editor;
pub mod folders;
mod layout;
mod library;
pub mod mutations;
pub mod navigation;
pub mod notes;
mod open_note;
#[cfg(test)]
mod persistence_tests;
mod picker;
pub mod pins;
pub mod ports;
pub mod search;
mod shell;
pub mod templates;

pub use shell::{WorkspaceAction, WorkspaceView};
