//! The rules of the meeting chat notice, apart from Tauri and Accessibility so they are tested with
//! a fake app and a real database. This is the only sender for app-owned recordings, whatever engine
//! opened them; the browser extension posts for the Meet calls it attaches to through the same
//! claim. One notice per account and session: the claim is written before the notice can be sent,
//! kept for seven days once it was or may have been sent, and given back only when it definitely was
//! not. Nothing is posted again after a send may have happened.

use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use anlg_audio::meeting::{CaptureAuthority, CaptureIdentity, NoticeOutcome};
use anlg_detect::{
    MeetingChatSendResult, MeetingPlatform,
    meeting_chat::{NativeMeetingApp, Outcome},
};
use tokio_util::sync::CancellationToken;

pub(crate) const MEETING_DISCLOSURE_MESSAGE_VERSION: &str = "anarlog-disclosure-v1";

/// Attempts at a recording with no confirmed call, a retry interval apart, while each one
/// definitely sent nothing: the call's chat may not be up yet when the recording starts.
pub(crate) const DISCOVERY_ATTEMPTS: usize = 30;

/// The confirmed call's app and process, where a notice may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NoticeTarget {
    pub(crate) app: NativeMeetingApp,
    pub(crate) pid: i32,
}

/// What a poster answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PostResult {
    Posted(Outcome),
    /// Refused before Return could be pressed (permission, platform, Accessibility failure).
    Refused(&'static str),
    /// No answer came back (the Accessibility thread stopped): Return may have been pressed.
    Lost,
}

/// One attempt of the sender that finds the call itself, with the platform it acted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiscoveredPost {
    pub(crate) result: PostResult,
    pub(crate) platform: &'static str,
}

impl DiscoveredPost {
    /// The Accessibility sender's answer. A failed press of the send control may still have sent
    /// the message, so it is unconfirmed rather than unsent.
    pub(crate) fn of(sent: &MeetingChatSendResult) -> Self {
        let outcome = if sent.sent {
            Outcome::Sent
        } else if sent.may_have_sent() {
            Outcome::Unconfirmed
        } else if sent.app.is_none() {
            Outcome::NoCall
        } else {
            Outcome::ChatUnavailable
        };
        let platform = match sent.platform {
            MeetingPlatform::Zoom => "zoom",
            MeetingPlatform::GoogleMeet => "google_meet",
            MeetingPlatform::MicrosoftTeams => "teams",
            MeetingPlatform::Slack => "slack_huddle",
            MeetingPlatform::Webex => "webex",
            MeetingPlatform::Discord | MeetingPlatform::Unknown => "unknown",
        };
        Self {
            result: PostResult::Posted(outcome),
            platform,
        }
    }

    fn cancelled(platform: &'static str) -> Self {
        Self {
            result: PostResult::Posted(Outcome::Cancelled),
            platform,
        }
    }

    /// Definitely sent nothing, and another attempt may find the chat.
    fn retryable(&self) -> bool {
        matches!(
            self.result,
            PostResult::Posted(outcome) if !outcome.may_have_sent() && outcome != Outcome::Cancelled
        )
    }
}

/// Asked on the Accessibility thread right before an attempt starts: the recording is still current
/// and nobody cancelled its notice.
pub(crate) type StillWanted = Box<dyn Fn() -> bool + Send + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Settlement {
    Sent,
    Unconfirmed,
}

/// The local evidence of one outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Delivery {
    pub(crate) platform: &'static str,
    pub(crate) message: String,
    pub(crate) delivery: &'static str,
    pub(crate) reason: &'static str,
    pub(crate) at_ms: i64,
}

/// How a notice ended, for the person: shown when it went wrong or may have gone out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoticeReport {
    pub(crate) capture: CaptureIdentity,
    pub(crate) outcome: NoticeOutcome,
    pub(crate) reason: &'static str,
}

/// The app around the rules: settings, the call settling, the posters.
pub(crate) trait NoticeHost: Send + Sync + 'static {
    /// The "post a disclosure in the meeting chat" setting.
    fn enabled(&self) -> bool;
    fn message(&self) -> String;
    fn now_ms(&self) -> i64;
    /// Time for the call's window to settle after the recording starts.
    fn settle(&self) -> impl Future<Output = ()> + Send;
    /// Posts once in the confirmed call. `cancel` reaches the native poster, which stops before
    /// Return.
    fn post(
        &self,
        target: NoticeTarget,
        message: String,
        cancel: CancellationToken,
    ) -> impl Future<Output = PostResult> + Send;
    /// One attempt of the Accessibility sender that finds the call among the apps using the
    /// microphone, for a recording with no confirmed call. It runs only if `wanted` still holds
    /// right before it starts; it can't be stopped once it has.
    fn post_discovered(
        &self,
        message: String,
        wanted: StillWanted,
    ) -> impl Future<Output = DiscoveredPost> + Send;
    /// The wait before another attempt of the discovering sender.
    fn retry_wait(&self) -> impl Future<Output = ()> + Send;
}

/// Durable claims and evidence; an error is a failure, never a claim.
pub(crate) trait NoticeStore: Send + Sync + 'static {
    fn claim(
        &self,
        capture: &CaptureIdentity,
        now_ms: i64,
    ) -> impl Future<Output = Result<bool, String>> + Send;
    fn settle(
        &self,
        capture: &CaptureIdentity,
        settlement: Settlement,
        now_ms: i64,
    ) -> impl Future<Output = Result<bool, String>> + Send;
    fn release(
        &self,
        capture: &CaptureIdentity,
    ) -> impl Future<Output = Result<bool, String>> + Send;
    fn record(
        &self,
        capture: &CaptureIdentity,
        delivery: Delivery,
    ) -> impl Future<Output = Result<(), String>> + Send;
}

impl NoticeStore for sqlx::SqlitePool {
    async fn claim(&self, capture: &CaptureIdentity, now_ms: i64) -> Result<bool, String> {
        anlg_db_app::claim_meeting_notice(
            self,
            &capture.account_id,
            &capture.session_id,
            &capture.capture_id,
            now_ms,
        )
        .await
        .map_err(|error| error.to_string())
    }

    async fn settle(
        &self,
        capture: &CaptureIdentity,
        settlement: Settlement,
        now_ms: i64,
    ) -> Result<bool, String> {
        let settlement = match settlement {
            Settlement::Sent => anlg_db_app::MeetingNoticeSettlement::Sent,
            Settlement::Unconfirmed => anlg_db_app::MeetingNoticeSettlement::Unconfirmed,
        };
        anlg_db_app::settle_meeting_notice(
            self,
            &capture.account_id,
            &capture.session_id,
            &capture.capture_id,
            settlement,
            now_ms,
        )
        .await
        .map_err(|error| error.to_string())
    }

    async fn release(&self, capture: &CaptureIdentity) -> Result<bool, String> {
        anlg_db_app::release_meeting_notice(
            self,
            &capture.account_id,
            &capture.session_id,
            &capture.capture_id,
        )
        .await
        .map_err(|error| error.to_string())
    }

    async fn record(&self, capture: &CaptureIdentity, delivery: Delivery) -> Result<(), String> {
        anlg_db_app::insert_disclosure_delivery(
            self,
            &anlg_db_app::DisclosureDelivery {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: capture.session_id.clone(),
                capture_id: capture.capture_id.clone(),
                attempted_at_ms: delivery.at_ms,
                platform: delivery.platform.to_string(),
                message_version: MEETING_DISCLOSURE_MESSAGE_VERSION.to_string(),
                message: delivery.message,
                delivery: delivery.delivery.to_string(),
                reason: delivery.reason.to_string(),
            },
        )
        .await
        .map_err(|error| error.to_string())
    }
}

/// Where a notice goes: the confirmed call, or wherever the discovering sender finds one.
enum Target {
    Call(NoticeTarget),
    Discovered,
}

/// The single owner of notice dispatch for app-owned recordings.
pub(crate) struct NoticeDispatcher<H, S> {
    host: H,
    store: S,
    /// The notice under way for each capture, so its end can cancel it.
    runs: Mutex<HashMap<String, CancellationToken>>,
}

impl<H: NoticeHost, S: NoticeStore> NoticeDispatcher<H, S> {
    pub(crate) fn new(host: H, store: S) -> Self {
        Self {
            host,
            store,
            runs: Mutex::default(),
        }
    }

    /// An app-owned recording started. Returns its notice work to spawn, once per capture. The
    /// durable claim, not this run, decides whether the session's notice may still be posted.
    pub(crate) fn start(
        self: &Arc<Self>,
        capture: CaptureIdentity,
        authority: Arc<dyn CaptureAuthority>,
    ) -> Option<impl Future<Output = Option<NoticeReport>> + Send + 'static> {
        let cancel = CancellationToken::new();
        {
            let mut runs = lock(&self.runs);
            if runs.contains_key(&capture.capture_id) {
                return None;
            }
            runs.insert(capture.capture_id.clone(), cancel.clone());
        }
        let this = Arc::clone(self);
        Some(async move {
            let report = this.run(&capture, &authority, &cancel).await;
            lock(&this.runs).remove(&capture.capture_id);
            report
        })
    }

    /// Stops the capture's notice before the send is pressed. Once it may have been pressed it is
    /// too late and the notice ends as it went.
    pub(crate) fn cancel(&self, capture: &CaptureIdentity) {
        if let Some(cancel) = lock(&self.runs).get(&capture.capture_id) {
            cancel.cancel();
        }
    }

    async fn run(
        &self,
        capture: &CaptureIdentity,
        authority: &Arc<dyn CaptureAuthority>,
        cancel: &CancellationToken,
    ) -> Option<NoticeReport> {
        if !self.host.enabled() || capture.account_id.trim().is_empty() {
            return None;
        }
        tokio::select! {
            _ = cancel.cancelled() => return None,
            _ = self.host.settle() => {}
        }
        let live = || !cancel.is_cancelled() && authority.is_current(capture);
        // Stopped, or someone else signed in, while the call settled: nothing was claimed. The
        // extension acts in its own Meet call; the setting may have been turned off meanwhile.
        if !live() || authority.extension_handles_actions(capture) || !self.host.enabled() {
            return None;
        }
        let message = self.host.message();
        // A confirmed call is the only place automated; without one, the discovering sender looks
        // for a call it can post in by itself, and refuses anything ambiguous.
        let target = match authority.confirmed_call(capture) {
            Some(call) => match NativeMeetingApp::from_bundle_id(&call.bundle_id) {
                Some(app) => Target::Call(NoticeTarget { app, pid: call.pid }),
                None => {
                    let reason = "extension_required";
                    self.record(capture, "unknown", &message, "not_sent", reason)
                        .await;
                    return Some(report(capture, NoticeOutcome::NotSent, reason));
                }
            },
            None => Target::Discovered,
        };
        let claimed = match self.store.claim(capture, self.host.now_ms()).await {
            Ok(claimed) => claimed,
            Err(error) => {
                tracing::warn!(%error, "meeting_notice_claim_failed");
                return Some(report(capture, NoticeOutcome::NotSent, "claim_unavailable"));
            }
        };
        // An earlier recording of the session, or the extension, holds the notice.
        if !claimed {
            return None;
        }
        // The claim write was asynchronous: the capture may have ended or changed hands meanwhile.
        if !live() || authority.extension_handles_actions(capture) {
            self.release(capture).await;
            let platform = match &target {
                Target::Call(target) => platform_of(target.app),
                Target::Discovered => "unknown",
            };
            self.record(capture, platform, &message, "cancelled", "")
                .await;
            return Some(report(capture, NoticeOutcome::CancelledBeforeDispatch, ""));
        }
        let (result, platform) = match target {
            Target::Call(target) => (
                self.host
                    .post(target, message.clone(), cancel.clone())
                    .await,
                platform_of(target.app),
            ),
            Target::Discovered => {
                let post = self
                    .post_discovered(capture, authority, &message, cancel)
                    .await;
                (post.result, post.platform)
            }
        };
        let (outcome, delivery, reason) = classify(result);
        if outcome.may_have_sent() {
            let settlement = if outcome == NoticeOutcome::Sent {
                Settlement::Sent
            } else {
                Settlement::Unconfirmed
            };
            // A failed write leaves the claim dispatching, which holds as firmly.
            if let Err(error) = self
                .store
                .settle(capture, settlement, self.host.now_ms())
                .await
            {
                tracing::warn!(%error, "meeting_notice_settle_failed");
            }
        } else {
            self.release(capture).await;
        }
        self.record(capture, platform, &message, delivery, reason)
            .await;
        Some(report(capture, outcome, reason))
    }

    /// Attempts the discovering sender until one may have sent the notice, nothing more can be
    /// tried, or the recording no longer wants it.
    async fn post_discovered(
        &self,
        capture: &CaptureIdentity,
        authority: &Arc<dyn CaptureAuthority>,
        message: &str,
        cancel: &CancellationToken,
    ) -> DiscoveredPost {
        let mut attempts = 0;
        loop {
            let wanted: StillWanted = {
                let (capture, authority, cancel) =
                    (capture.clone(), authority.clone(), cancel.clone());
                Box::new(move || {
                    !cancel.is_cancelled()
                        && authority.is_current(&capture)
                        && !authority.extension_handles_actions(&capture)
                })
            };
            let post = self.host.post_discovered(message.to_string(), wanted).await;
            attempts += 1;
            if !post.retryable() || attempts >= DISCOVERY_ATTEMPTS {
                return post;
            }
            // A stop or sign-out meanwhile reaches the next attempt through `wanted`.
            tokio::select! {
                _ = cancel.cancelled() => return DiscoveredPost::cancelled(post.platform),
                _ = self.host.retry_wait() => {}
            }
        }
    }

    /// The extension's claim on the notice of the capture it was handed, in the same durable store,
    /// so the extension and this dispatcher never both post a session's notice: the disclosure to
    /// post, or `None` when the setting is off, the capture is no longer current, an earlier notice
    /// holds the claim, or the claim could not be written.
    pub(crate) async fn claim_for_extension(
        &self,
        capture: &CaptureIdentity,
        authority: &dyn CaptureAuthority,
    ) -> Option<String> {
        if !self.host.enabled()
            || capture.account_id.trim().is_empty()
            || !authority.is_current(capture)
        {
            return None;
        }
        match self.store.claim(capture, self.host.now_ms()).await {
            Ok(true) => {}
            Ok(false) => return None,
            Err(error) => {
                tracing::warn!(%error, "meeting_notice_claim_failed");
                return None;
            }
        }
        if !authority.is_current(capture) {
            self.release(capture).await;
            return None;
        }
        Some(self.host.message())
    }

    /// Settles the extension's claim with how its notice went: kept once it was or may have been
    /// sent, given back only when it definitely was not.
    pub(crate) async fn settle_for_extension(
        &self,
        capture: &CaptureIdentity,
        outcome: NoticeOutcome,
        reason: &'static str,
    ) {
        let delivery = match outcome {
            NoticeOutcome::Sent | NoticeOutcome::Unconfirmed => {
                let (settlement, delivery) = if outcome == NoticeOutcome::Sent {
                    (Settlement::Sent, "sent")
                } else {
                    (Settlement::Unconfirmed, "unconfirmed")
                };
                if let Err(error) = self
                    .store
                    .settle(capture, settlement, self.host.now_ms())
                    .await
                {
                    tracing::warn!(%error, "meeting_notice_settle_failed");
                }
                delivery
            }
            NoticeOutcome::NotSent => {
                self.release(capture).await;
                "not_sent"
            }
            NoticeOutcome::CancelledBeforeDispatch => {
                self.release(capture).await;
                "cancelled"
            }
        };
        let message = self.host.message();
        self.record(capture, "google_meet", &message, delivery, reason)
            .await;
    }

    async fn release(&self, capture: &CaptureIdentity) {
        // A claim that can't be given back stays: a later recording then posts nothing.
        if let Err(error) = self.store.release(capture).await {
            tracing::warn!(%error, "meeting_notice_release_failed");
        }
    }

    async fn record(
        &self,
        capture: &CaptureIdentity,
        platform: &'static str,
        message: &str,
        delivery: &'static str,
        reason: &'static str,
    ) {
        let delivery = Delivery {
            platform,
            message: message.to_string(),
            delivery,
            reason,
            at_ms: self.host.now_ms(),
        };
        if let Err(error) = self.store.record(capture, delivery).await {
            tracing::warn!(%error, "meeting_notice_evidence_failed");
        }
    }
}

/// The shared outcome, the evidence's delivery value, and the reason.
fn classify(result: PostResult) -> (NoticeOutcome, &'static str, &'static str) {
    match result {
        PostResult::Posted(Outcome::Sent) => (NoticeOutcome::Sent, "sent", ""),
        PostResult::Posted(Outcome::Unconfirmed) => {
            (NoticeOutcome::Unconfirmed, "unconfirmed", "unconfirmed")
        }
        PostResult::Posted(Outcome::Cancelled) => {
            (NoticeOutcome::CancelledBeforeDispatch, "cancelled", "")
        }
        PostResult::Posted(outcome) => (NoticeOutcome::NotSent, "not_sent", outcome.as_str()),
        PostResult::Refused(reason) => (NoticeOutcome::NotSent, "not_sent", reason),
        PostResult::Lost => (NoticeOutcome::Unconfirmed, "unconfirmed", "no_answer"),
    }
}

fn platform_of(app: NativeMeetingApp) -> &'static str {
    match app {
        NativeMeetingApp::Zoom => "zoom",
        NativeMeetingApp::Teams => "teams",
        NativeMeetingApp::Webex => "webex",
        NativeMeetingApp::Slack => "slack_huddle",
    }
}

fn report(capture: &CaptureIdentity, outcome: NoticeOutcome, reason: &'static str) -> NoticeReport {
    NoticeReport {
        capture: capture.clone(),
        outcome,
        reason,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
