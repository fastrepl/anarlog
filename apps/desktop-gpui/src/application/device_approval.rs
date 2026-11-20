//! Shipping's `startAutomaticDeviceEnrollment`: while this device syncs, it checks every 5 seconds
//! for the account's other devices waiting for approval and approves them, whether or not the
//! Sync page is open.

use std::time::Duration;

use gpui::Context;

use super::ApplicationView;
use crate::product::cloud::{CloudServices, auth::SecureAuthProvider};

const POLL: Duration = Duration::from_secs(5);

pub(super) fn watch(cloud: CloudServices, cx: &mut Context<ApplicationView>) {
    cx.spawn(async move |this, cx| {
        loop {
            let paused = this
                .update(cx, |this, _| this.writers_paused || this.closing)
                .unwrap_or(true);
            if !paused
                && cloud.identity().account_id.is_some()
                && let Err(error) = cloud.approve_devices().await
            {
                // Approval retries on the next poll; never log key material.
                tracing::debug!(%error, "pending devices were not approved");
            }
            cx.background_executor().timer(POLL).await;
            if this.upgrade().is_none() {
                break;
            }
        }
    })
    .detach();
}
