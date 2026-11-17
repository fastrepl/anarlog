//! Shipping's `WorkspaceInvitationToasts`: checks the account's workspace invitations every
//! 60 seconds and announces each new one once per launch.

use std::{collections::HashSet, sync::Arc, time::Duration};

use gpui::Context;

use super::ApplicationView;
use crate::product::{
    account,
    cloud::{
        calls,
        teams::client::{self, MyInvitation},
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
                        for gone in announced.difference(&current) {
                            dismiss(gone);
                        }
                        for invitation in &invitations {
                            if !announced.contains(&invitation.invitation_id) {
                                announce(invitation);
                            }
                        }
                        announced = current;
                    }
                }
                None => {
                    for gone in announced.drain() {
                        dismiss(&gone);
                    }
                }
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
fn announce(invitation: &MyInvitation) {
    let id = toast_id(&invitation.invitation_id);
    let title = format!("You've been invited to join {}", invitation.workspace_name);
    let description = invitation
        .invited_by_email
        .as_ref()
        .map(|email| format!("Invited by {email}"));
    // INTEGRATION toasts::show(..): id `id`, `title`, `description`, duration Infinity, action
    // "View" → open Settings at "team" and dismiss `id`.
    // INTEGRATION MeetingAutomation::team_invitation(..): key `id`, `title`, `description` or "",
    // 15 s timeout, action "View"; skipped when notifications are disabled or the window is active.
    let _ = (id, title, description);
}

fn dismiss(invitation: &str) {
    // INTEGRATION toasts::dismiss(&toast_id(invitation)).
    let _ = toast_id(invitation);
}
