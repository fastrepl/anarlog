//! Shipping's note sharing (`session-sharing/*`), shared-note views (`shared-notes/*`) and
//! the folder and template share dialog (`resource-sharing/*`).

pub mod assets;
pub mod folder_access;
mod model;
pub mod popover;
pub mod resource_dialog;
pub mod shared_note;

use gpui::Global;

pub use model::*;
pub use popover::{SharePopover, SharePopoverEvent, anchor};

use super::cloud::CloudServices;

/// The signed-in cloud service the share surfaces call; absent until cloud sign-in is set up.
#[derive(Clone)]
pub struct SharingCloud(pub CloudServices);

impl Global for SharingCloud {}
