//! What the shipping app kept only in its web view's localStorage, as its bridge release mirrored
//! it (`anlg_desktop_identity::webview_storage`). A channel run merges it into this app's own
//! state at launch, once the replaced app has exited: Cloud API progress and the accounts whose
//! initial sync was already announced. Trial dialogs (`product/trial.rs`) and the zoom factor
//! (`main.rs`) read the mirror where they load.

use std::path::Path;

use anlg_desktop_identity::webview_storage::{CLOUDSYNC_INITIAL_SYNC_COMPLETED, WebviewStorage};

use crate::{meeting::automation, product::cloud::cloud_api};

pub fn import(storage: &WebviewStorage, root: &Path) {
    let mirror = match storage.read() {
        Ok(Some(mirror)) => mirror,
        Ok(None) => return,
        Err(error) => {
            tracing::warn!(%error, "the shipping app's localStorage mirror is unreadable");
            return;
        }
    };
    if let Err(error) = cloud_api::import_shipping(&root.join(cloud_api::PROGRESS_FILE), &mirror) {
        tracing::warn!(%error, "the shipping app's Cloud API progress was not imported");
    }
    for (user, completed) in mirror.with_prefix(CLOUDSYNC_INITIAL_SYNC_COMPLETED) {
        if completed != "1" || automation::cloudsync_announced(root, user) {
            continue;
        }
        if let Err(error) = automation::mark_cloudsync_announced(root, user) {
            tracing::warn!(%error, "the shipping app's initial sync notice was not imported");
        }
    }
}
