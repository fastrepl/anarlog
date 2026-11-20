//! Shipping's `DurableSharedNoteCacheSync`: reconciles the shared-note cache every 60 seconds
//! while signed in, so notes unshared since they were cached stop opening.

use std::time::Duration;

use desktop_runtime::CancellationToken;
use gpui::Context;

use super::ApplicationView;
use crate::product::{
    cloud::{CloudServices, auth::SecureAuthProvider},
    sharing::shared_note::SharedNotesSynced,
};

const REFRESH: Duration = Duration::from_secs(60);

pub(super) fn watch(cloud: CloudServices, cx: &mut Context<ApplicationView>) {
    cx.spawn(async move |this, cx| {
        loop {
            let paused = this
                .update(cx, |this, _| this.writers_paused || this.closing)
                .unwrap_or(true);
            if !paused && cloud.identity().account_id.is_some() {
                match cloud.reconcile_shared(CancellationToken::new()).await {
                    Ok(()) => cx.update(|cx| cx.default_global::<SharedNotesSynced>().0 += 1),
                    Err(error) => tracing::warn!(%error, "shared notes were not refreshed"),
                }
            }
            cx.background_executor().timer(REFRESH).await;
            if this.upgrade().is_none() {
                break;
            }
        }
    })
    .detach();
}
