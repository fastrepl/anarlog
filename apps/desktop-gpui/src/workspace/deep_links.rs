//! `useDeeplinkHandler` and the `Join & record` header flow
//! (`HeaderMeetingControl`'s `joinMeeting`).

use gpui::Context;

use super::Workspace;
use crate::deeplink::DeepLink;
use crate::timeline::SessionEvent;

impl Workspace {
    /// `handleDeepLink`: the routed deep links. Auth, billing, integration
    /// callbacks and shared-note opens belong to the signed-in flows, which
    /// the native shell does not implement yet; they are logged so a stray
    /// link never silently disappears.
    pub(crate) fn handle_deep_link(&mut self, link: DeepLink, cx: &mut Context<Self>) {
        match link {
            DeepLink::OnboardingDemoComplete(_) => self.stop_active_welcome_demo(cx),
            DeepLink::AuthCallback(_) | DeepLink::BillingRefresh(_) => {
                tracing::warn!(
                    path = link.path(),
                    "deep link needs the account flows, which the native shell does not ship yet"
                );
            }
            DeepLink::IntegrationCallback(search) => {
                tracing::warn!(
                    integration_id = %search.integration_id,
                    status = %search.status,
                    "integration callbacks need the account flows, which the native shell does not ship yet"
                );
            }
        }
    }

    /// `stopActiveWelcomeDemo`: when the live capture is the onboarding demo
    /// session, stop listening once the demo video reports completion.
    fn stop_active_welcome_demo(&mut self, cx: &mut Context<Self>) {
        let Some(session_id) = self
            .recording
            .live
            .as_ref()
            .map(|live| live.session_id.clone())
        else {
            return;
        };
        let task = self.store.is_welcome_session(session_id.clone());
        cx.spawn(async move |this, cx| {
            let is_demo = match task.await {
                Ok(Ok(is_demo)) => is_demo,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "[onboarding] failed to check the live session");
                    false
                }
                Err(error) => {
                    tracing::warn!(%error, "[onboarding] failed to check the live session");
                    false
                }
            };
            if !is_demo {
                return;
            }
            this.update(cx, |this, cx| {
                // The capture may have changed while the query ran.
                if this
                    .recording
                    .live
                    .as_ref()
                    .is_some_and(|live| live.session_id == session_id)
                {
                    this.stop_listening(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `joinMeeting`: open the meeting link and start listening at the same
    /// time. The onboarding demo gets the autoplay URL with a loopback
    /// completion callback (`startCallbackServer`), everything else opens
    /// the link as is.
    pub(crate) fn join_meeting(
        &mut self,
        session_id: String,
        event: &SessionEvent,
        cx: &mut Context<Self>,
    ) {
        if self.joining_meeting {
            return;
        }
        let Some(link) = event
            .meeting_link
            .clone()
            .filter(|link| !link.trim().is_empty())
        else {
            return;
        };
        self.joining_meeting = true;
        cx.notify();

        if event.is_welcome_demo() {
            let server_start = cx.global::<crate::DeepLinks>().server.start_task();
            cx.spawn(async move |this, cx| {
                let port = match server_start.await {
                    Ok(Ok(port)) => Some(port),
                    Ok(Err(error)) => {
                        tracing::error!(%error, "[onboarding] failed to prepare demo completion callback");
                        None
                    }
                    Err(error) => {
                        tracing::error!(%error, "[onboarding] failed to prepare demo completion callback");
                        None
                    }
                };
                let url = crate::deeplink::welcome_demo_url(&link, port);
                this.update(cx, |this, cx| {
                    crate::opener::open_url(&url);
                    this.start_listening(session_id, cx);
                    this.joining_meeting = false;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        } else {
            crate::opener::open_url(&link);
            self.start_listening(session_id, cx);
            self.joining_meeting = false;
            cx.notify();
        }
    }
}
