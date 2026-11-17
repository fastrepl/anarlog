//! Shipping's `WorkspaceInvitationToasts`: checks the account's workspace invitations every
//! 60 seconds and announces each new one once per launch.

use std::{collections::HashSet, sync::Arc, time::Duration};

use gpui::{App, Context, WeakEntity};

use super::{ApplicationView, toasts};
use crate::{
    meeting::automation::{MeetingAutomation, Request},
    product::{
        account,
        cloud::{
            calls,
            teams::client::{self, MyInvitation},
        },
    },
};

const POLL: Duration = Duration::from_secs(60);

pub fn toast_id(invitation: &str) -> String {
    format!("team-invitation:{invitation}")
}

pub(super) fn watch(cx: &mut Context<ApplicationView>) {
    cx.spawn(async move |this, cx| {
        let mut announced = HashSet::<Arc<str>>::new();
        loop {
            let calls = cx.update(|cx| {
                account::current(cx)
                    .signed_in()
                    .then(|| calls::installed(cx))
                    .flatten()
            });
            match calls {
                Some(calls) => {
                    if let Ok(invitations) = client::list_my_invitations(&*calls).await {
                        let current = invitations
                            .iter()
                            .map(|invitation| invitation.invitation_id.clone())
                            .collect::<HashSet<_>>();
                        cx.update(|cx| {
                            for gone in announced.difference(&current) {
                                dismiss(gone, cx);
                            }
                            for invitation in &invitations {
                                if !announced.contains(&invitation.invitation_id) {
                                    announce(invitation, &this, cx);
                                }
                            }
                        });
                        announced = current;
                    }
                }
                None => cx.update(|cx| {
                    for gone in announced.drain() {
                        dismiss(&gone, cx);
                    }
                }),
            }
            cx.background_executor().timer(POLL).await;
            if this.upgrade().is_none() {
                break;
            }
        }
    })
    .detach();
}

/// The invitation's toast, which never times out and whose View opens Teams settings, and the
/// OS notification shipping posts when the app window is inactive.
fn announce(invitation: &MyInvitation, view: &WeakEntity<ApplicationView>, cx: &mut App) {
    let mut toast = toasts::Toast::info(format!(
        "You've been invited to join {}",
        invitation.workspace_name
    ))
    .id(toast_id(&invitation.invitation_id))
    .sticky();
    if let Some(email) = &invitation.invited_by_email {
        toast = toast.description(format!("Invited by {email}"));
    }
    let view = view.clone();
    toasts::show(
        toast.action("View", move |window, cx| {
            let _ = view.update(cx, |this, cx| {
                this.automation_request(&Request::OpenSettings("team"), window, cx)
            });
        }),
        cx,
    );
    if let Some(automation) = MeetingAutomation::global(cx) {
        automation.update(cx, |automation, _| {
            automation.team_invitation(
                &invitation.invitation_id,
                &invitation.workspace_name,
                invitation.invited_by_email.as_deref(),
            )
        });
    }
}

fn dismiss(invitation: &str, cx: &mut App) {
    toasts::dismiss(&toast_id(invitation), cx);
}
