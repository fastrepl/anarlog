use std::collections::HashSet;

use desktop_runtime::Result;
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::{Value, json};

use super::{CloudServices, Core, auth::AuthLease, transport::text};
use crate::product::account::{Account, Claims, Devices, User, Workspace, WorkspaceTier, now};

impl CloudServices {
    /// The signed-in account with its plan, workspace seats, trial offer and synced devices.
    pub fn account(&self) -> BoxFuture<'static, Result<Account>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |_| async move { this.core.account().await })?
                .receive()
                .await
        })
    }
}

impl Core {
    async fn account(&self) -> Result<Account> {
        let identity = self.auth.identity();
        let Some(id) = identity.account_id.clone() else {
            return Ok(Account::default());
        };
        let lease = self.auth.lease(&id, false).await?;
        let claims = Claims::decode(lease.access_token()?).unwrap_or_default();
        // Workspaces only add access, so an unreadable directory fails closed to none.
        let workspaces = self.workspace_seats(&lease).await.unwrap_or_default();
        let billing = claims.billing(now());
        let personal = workspaces.iter().all(|w| w.tier == WorkspaceTier::Free);
        let trial_eligible = !billing.is_paid
            && personal
            && self
                .api(&lease, Method::GET, "/subscription/can-start-trial", None)
                .await
                .is_ok_and(|value| value["canStartTrial"] == true);
        let devices = if billing.is_pro {
            self.api(&lease, Method::GET, "/sync/devices", None)
                .await
                .ok()
                .and_then(|value| devices(&value))
        } else {
            None
        };
        lease.check()?;
        Ok(Account {
            user: Some(User {
                id,
                email: identity.email,
            }),
            claims: Some(claims),
            workspaces: Some(workspaces.into()),
            trial_eligible,
            devices,
        })
    }

    async fn workspace_seats(&self, lease: &AuthLease) -> Result<Vec<Workspace>> {
        let memberships = self.memberships(lease).await?;
        let mut seats = Vec::new();
        for membership in memberships.as_array().into_iter().flatten() {
            if membership["workspaces"]["kind"] != "shared" {
                continue;
            }
            let id = text(membership, "workspace_id")?;
            let access = self
                .rpc(lease, "get_workspace_access", json!({"p_workspace_id": id}))
                .await?;
            let access = super::transport::first(&access)?;
            let Some(tier) = access["workspace_tier"]
                .as_str()
                .and_then(WorkspaceTier::parse)
            else {
                continue;
            };
            seats.push(Workspace {
                id: id.into(),
                name: membership["workspaces"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                tier,
                seat_limit: access["seat_limit"].as_u64().map(|limit| limit as u32),
                used_seats: access["used_seats"].as_u64().unwrap_or(0) as u32,
            });
        }
        Ok(seats)
    }
}

/// Devices and pending enrollments both hold a slot, counted once per fingerprint.
fn devices(value: &Value) -> Option<Devices> {
    let max = value["maxDevices"].as_u64()? as u32;
    let used = ["devices", "pendingDevices"]
        .iter()
        .flat_map(|key| value[*key].as_array().into_iter().flatten())
        .filter_map(|device| device["deviceFingerprint"].as_str())
        .collect::<HashSet<_>>()
        .len() as u32;
    (max > 0).then_some(Devices { used, max })
}

impl CloudServices {
    /// Opens an Anarlog web page in the browser, like shipping's `buildWebAppUrl`: later
    /// parameters replace earlier ones of the same name.
    pub fn open_web(
        &self,
        path: &'static str,
        params: Vec<(&'static str, String)>,
        fragment: Option<&'static str>,
    ) -> BoxFuture<'static, Result<()>> {
        let core = self.core.clone();
        Box::pin(async move {
            let mut url = core.auth.transport.config.web_flow(path)?;
            let mut pairs = url
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect::<Vec<_>>();
            for (key, value) in params {
                pairs.retain(|(name, _)| name != key);
                pairs.push((key.to_owned(), value));
            }
            url.query_pairs_mut().clear().extend_pairs(pairs);
            url.set_fragment(fragment);
            core.host.open_url(url).await
        })
    }

    /// Loads the account and publishes it app-wide, unless the signed-in user changed meanwhile.
    pub fn publish_account(&self, cx: &mut gpui::App) {
        let this = self.clone();
        cx.spawn(async move |cx| {
            let expected = this.core.auth.identity().account_id;
            if let Ok(account) = this.account().await {
                cx.update(|cx| {
                    if this.core.auth.identity().account_id == expected {
                        crate::product::account::set(account, cx);
                    }
                });
            }
        })
        .detach();
    }
}
