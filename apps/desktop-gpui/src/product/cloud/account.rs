use std::collections::HashSet;

use desktop_runtime::Result;
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::{Value, json};

use super::{CloudServices, Core, auth::AuthLease, transport::text};
use crate::product::account::{
    Account, Claims, Devices, Recorder, User, Workspace, WorkspaceTier, now,
};

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
                .and_then(|value| devices(&value, &self.auth.transport.config.device_fingerprint))
        } else {
            None
        };
        lease.check()?;
        Ok(Account {
            user: Some(User {
                id,
                email: identity.email,
                created_at: identity.created_at,
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

/// `get_referral_summary`, as `settings/referrals.tsx` validates it.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Referrals {
    pub enabled: bool,
    pub eligible: bool,
    pub code: Option<String>,
    pub accepted: u32,
    pub remaining: u32,
    pub months_earned: u32,
    pub invites: Vec<ReferralInvite>,
    /// The invite link built from `code` and the web app URL.
    #[serde(skip)]
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct ReferralInvite {
    pub id: String,
    pub slot: u32,
    pub policy: String,
    pub accepted_at: String,
    pub extended_until: Option<String>,
    pub legacy_amount_cents: i64,
    pub status: String,
}

impl CloudServices {
    pub fn referrals(&self) -> BoxFuture<'static, Result<Referrals>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |_| async move { this.core.referrals().await })?
                .receive()
                .await
        })
    }
}

impl Core {
    async fn referrals(&self) -> Result<Referrals> {
        let id = self
            .auth
            .identity()
            .account_id
            .clone()
            .ok_or_else(|| desktop_runtime::ServiceError::Failed("Unauthorized".into()))?;
        let lease = self.auth.lease(&id, false).await?;
        let value = self.rpc(&lease, "get_referral_summary", json!({})).await?;
        lease.check()?;
        let mut summary: Referrals = serde_json::from_value(value).map_err(|_| {
            desktop_runtime::ServiceError::Failed("Invalid referral summary".into())
        })?;
        summary.code = summary.code.filter(|code| {
            code.len() == 24
                && code
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
        summary.link = summary.code.as_ref().and_then(|code| {
            self.auth
                .transport
                .config
                .web
                .join(&format!("/invite/{code}"))
                .ok()
                .map(String::from)
        });
        Ok(summary)
    }
}

/// Records an automatic trial attempt for `account` (`claim`) or releases it after a failure
/// so a later refresh retries, like shipping's `startTrialOnce`.
fn claim_trial_start(account: &str, claim: bool) -> bool {
    static ATTEMPTED: std::sync::Mutex<std::collections::BTreeSet<String>> =
        std::sync::Mutex::new(std::collections::BTreeSet::new());
    let mut attempted = ATTEMPTED.lock().unwrap_or_else(|error| error.into_inner());
    if claim {
        attempted.insert(account.to_owned())
    } else {
        attempted.remove(account)
    }
}

impl Core {
    async fn start_monthly_trial(&self) -> Result<bool> {
        let Some(id) = self.auth.identity().account_id.clone() else {
            return Ok(false);
        };
        let lease = self.auth.lease(&id, false).await?;
        let response = self
            .api(
                &lease,
                Method::POST,
                "/subscription/start-trial?interval=monthly",
                None,
            )
            .await?;
        let started = response["started"] == true;
        if started {
            // The trial is in the next token's claims.
            self.auth.lease(&id, true).await?;
        }
        Ok(started)
    }
}

/// Synced devices for the current account.
fn devices(value: &Value, fingerprint: &str) -> Option<Devices> {
    let max = value["maxDevices"].as_u64().unwrap_or(0) as u32;
    let recorder = value["devices"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|device| device["deviceFingerprint"] == fingerprint)
        .map(|device| Recorder {
            fingerprint: fingerprint.into(),
            name: device["deviceName"]
                .as_str()
                .filter(|name| !name.is_empty())
                .unwrap_or("Unnamed device")
                .into(),
        });
    let used = ["devices", "pendingDevices"]
        .iter()
        .flat_map(|key| value[*key].as_array().into_iter().flatten())
        .filter_map(|device| device["deviceFingerprint"].as_str())
        .collect::<HashSet<_>>()
        .len() as u32;
    Some(Devices {
        used,
        max,
        recorder,
    })
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
            // Shipping opens every checkout and billing-portal page with the hand-off card.
            if matches!(path, "/app/checkout" | "/app/portal" | "/app/team-checkout") {
                core.host.hand_off(url, super::host::Handoff::Billing).await
            } else {
                core.host.open_url(url).await
            }
        })
    }

    /// Loads the account and publishes it app-wide, unless the signed-in user changed meanwhile.
    pub fn publish_account(&self, cx: &mut gpui::App) {
        let this = self.clone();
        cx.spawn(async move |cx| {
            let expected = this.core.auth.identity().account_id;
            let Ok(account) = this.account().await else {
                return;
            };
            let start = account.trial_eligible
                && expected
                    .as_deref()
                    .is_some_and(|id| claim_trial_start(id, true));
            let publish = |account, cx: &mut gpui::AsyncApp| {
                cx.update(|cx| {
                    if this.core.auth.identity().account_id == expected {
                        crate::product::account::set(account, cx);
                    }
                })
            };
            publish(account, cx);
            if !start {
                return;
            }
            // Shipping's `startEligibleTrial`: one automatic monthly trial per account.
            let started = this
                .runtime
                .service({
                    let core = this.core.clone();
                    move |_| async move { core.start_monthly_trial().await }
                })
                .map(|reply| reply.receive());
            let started = match started {
                Ok(reply) => reply.await,
                Err(error) => Err(error),
            };
            match started {
                Ok(true) => {
                    if let Ok(account) = this.account().await {
                        publish(account, cx);
                    }
                }
                _ => {
                    if let Some(id) = expected.as_deref() {
                        claim_trial_start(id, false);
                    }
                }
            }
        })
        .detach();
    }
}
