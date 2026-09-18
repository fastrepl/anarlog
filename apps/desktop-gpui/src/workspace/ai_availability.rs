//! `useProviderAvailability`: whether a provider that must prove itself —
//! a local server to reach or an API key to verify — currently does, cached
//! per config and re-polled for local servers while an AI page is open.

use std::collections::HashMap;
use std::time::Duration;

use gpui::Context;

use super::Workspace;
use super::ai_settings::ProviderKind;
use crate::ai_providers::Provider;
use crate::ai_verify::{
    check_local_availability, credential_identity, verify_provider_credentials,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Availability {
    Pending,
    Known(bool),
}

pub(super) type AvailabilityCache = HashMap<String, Availability>;

/// `requiresKeyVerification`
pub(super) fn requires_key_verification(provider: &Provider) -> bool {
    !provider.subscription
        && !provider.checks_availability
        && provider.required_fields().contains(&"api_key")
}

fn cache_key(kind: ProviderKind, provider: &Provider, base_url: &str, api_key: &str) -> String {
    format!(
        "{}\u{1}{}\u{1}{}\u{1}{}",
        kind.key(),
        provider.id,
        base_url,
        credential_identity(provider.id, base_url, api_key)
    )
}

impl Workspace {
    /// `availability[provider.id]`: `None` while unknown or pending.
    pub(super) fn ai_provider_available(
        &self,
        kind: ProviderKind,
        provider: &Provider,
    ) -> Option<bool> {
        let (base_url, api_key) = self.ai_provider_config(kind, provider);
        match self
            .ai_availability
            .get(&cache_key(kind, provider, &base_url, &api_key))
        {
            Some(Availability::Known(available)) => Some(*available),
            _ => None,
        }
    }

    /// Run the checks the page needs: every provider with `checkAvailability`
    /// or key verification whose config has no blockers. `force` re-runs the
    /// local-server reachability checks (`refetchInterval: 5_000`).
    pub(super) fn ensure_ai_availability(
        &mut self,
        kind: ProviderKind,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        for provider in kind.providers() {
            if provider.disabled
                || !(provider.checks_availability || requires_key_verification(provider))
            {
                continue;
            }
            if !self.ai_provider_config_complete(kind, provider) {
                continue;
            }
            let (base_url, api_key) = self.ai_provider_config(kind, provider);
            let key = cache_key(kind, provider, &base_url, &api_key);
            let rerun = force && provider.checks_availability;
            if self.ai_availability.contains_key(&key) && !rerun {
                continue;
            }
            if !self.ai_availability.contains_key(&key) {
                self.ai_availability
                    .insert(key.clone(), Availability::Pending);
            }
            let provider_id = provider.id;
            let local = provider.checks_availability;
            let credential_kind = kind.credential_kind();
            let task = self.store.runtime().spawn(async move {
                if local {
                    check_local_availability(provider_id, &base_url, &api_key).await
                } else {
                    // A retryable failure leaves the query without data
                    // (`retry: false`), which reads as unavailable too.
                    verify_provider_credentials(credential_kind, provider_id, &base_url, &api_key)
                        .await
                        .is_ok()
                }
            });
            cx.spawn(async move |this, cx| {
                let available = task.await.unwrap_or(false);
                this.update(cx, |this, cx| {
                    let changed = this
                        .ai_availability
                        .insert(key, Availability::Known(available))
                        != Some(Availability::Known(available));
                    if changed {
                        match kind {
                            ProviderKind::Llm => this.ensure_llm_models(false, cx),
                            // `PersistAiSelection` once a provider becomes usable.
                            ProviderKind::Stt => this.apply_default_stt_selection(cx),
                        }
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
        self.start_availability_poll(kind, cx);
    }

    /// The 5s `refetchInterval` of the local-server checks while the page
    /// is open.
    fn start_availability_poll(&mut self, kind: ProviderKind, cx: &mut Context<Self>) {
        if self.availability_polls.contains(&kind) {
            return;
        }
        self.availability_polls.insert(kind);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                let keep_going = this
                    .update(cx, |this, cx| {
                        let open = this.settings_tab
                            == Some(match kind {
                                ProviderKind::Stt => super::settings::SettingsTab::Transcription,
                                ProviderKind::Llm => super::settings::SettingsTab::Intelligence,
                            });
                        if open {
                            this.ensure_ai_availability(kind, true, cx);
                        } else {
                            this.availability_polls.remove(&kind);
                        }
                        open
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }
}
