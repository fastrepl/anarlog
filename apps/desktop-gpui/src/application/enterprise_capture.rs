//! Shipping's `EnterpriseCaptureSync`: polls enterprise capture delivery every 15 seconds while
//! signed in, then runs each delivered meeting's meeting-completed automations.

use gpui::Context;

use super::ApplicationView;
use crate::{
    product::cloud::{CloudServices, auth::SecureAuthProvider, enterprise_capture},
    workspace::automation_runner::Trigger,
};

pub(super) fn watch(cloud: CloudServices, cx: &mut Context<ApplicationView>) {
    let Some(server) = enterprise_capture::server() else {
        return;
    };
    cx.spawn(async move |this, cx| {
        loop {
            let paused = this
                .update(cx, |this, _| this.writers_paused || this.closing)
                .unwrap_or(true);
            if !paused && cloud.identity().account_id.is_some() {
                match cloud.deliver_captures(server.clone()).await {
                    Ok(delivery) => {
                        if let Some(error) = delivery.error {
                            tracing::warn!(%error, "enterprise capture delivery failed");
                        }
                        for completion in delivery.completions {
                            // Shipping also emits the local API's `meeting.completed` event,
                            // which the native app has no server for yet.
                            let started = this.update(cx, |this, cx| {
                                this.workspace.update(cx, |workspace, cx| {
                                    workspace.run_automations(
                                        Trigger::MeetingCompleted,
                                        completion.session.clone(),
                                        cx,
                                    )
                                })
                            });
                            if started.is_err() {
                                return;
                            }
                            if let Err(error) = cloud.complete_capture(completion.source_id).await {
                                tracing::warn!(%error, "captured meeting was not marked complete");
                            }
                        }
                    }
                    Err(error) => tracing::warn!(%error, "enterprise capture delivery failed"),
                }
            }
            cx.background_executor()
                .timer(enterprise_capture::POLL_INTERVAL)
                .await;
            if this.upgrade().is_none() {
                break;
            }
        }
    })
    .detach();
}
