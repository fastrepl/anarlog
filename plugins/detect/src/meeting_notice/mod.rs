//! Meeting chat notice delivery, run from the meeting runtime's lifecycle: the one sender for every
//! app-owned recording, whatever engine opened it. Provenance: crates/detect/PROVENANCE.md.

mod dispatch;

use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anlg_audio::meeting::{
    CaptureAuthority, CaptureIdentity, LifecycleOwner, MeetingLifecycleEvent,
    MeetingLifecycleObserver, NoticeOutcome,
};
use anlg_detect::meeting_chat;
use tauri::{AppHandle, EventTarget, Manager, Runtime};
use tauri_plugin_windows::WindowImpl;
use tauri_specta::Event;
use tokio_util::sync::CancellationToken;

use crate::{DetectPluginExt, ax_worker::AxWorker};
use dispatch::{
    Delivery, DiscoveredPost, NoticeDispatcher, NoticeHost, NoticeReport, NoticeStore,
    NoticeTarget, PostResult, Settlement, StillWanted,
};

/// The disclosure Anarlog posts in the meeting chat. Chat capture ignores it as a consent reply.
pub const MEETING_DISCLOSURE_MESSAGE: &str =
    "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so";

/// Time for the call's window to settle after the recording starts, before the chat is looked for.
const SETTLE: Duration = Duration::from_secs(4);
const POST_TIMEOUT: Duration = Duration::from_secs(8);
/// Between attempts of the sender that finds the call itself.
const DISCOVERY_RETRY: Duration = Duration::from_secs(1);

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MeetingNoticeOutcome {
    Sent,
    NotSent,
    CancelledBeforeDispatch,
    /// Return was pressed but the message wasn't seen leaving the input: it may be in the chat,
    /// and it is never posted again.
    Unconfirmed,
}

/// How a recording's meeting chat notice ended, for the window that tells the person.
#[derive(serde::Serialize, Clone, Debug, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct MeetingNoticeEvent {
    pub session_id: String,
    pub capture_id: String,
    pub outcome: MeetingNoticeOutcome,
    /// Why nothing was sent, such as `no_call`, `chat_unavailable`, `draft_in_progress`,
    /// `permission_required` or `claim_unavailable`; empty when sent.
    pub reason: String,
}

/// Registered with the meeting runtime. Posts each app-owned recording's notice, and cancels it,
/// down to the native poster, when the recording ends or a browser extension takes the call.
pub struct MeetingNoticeObserver<R: Runtime> {
    app: AppHandle<R>,
    dispatcher: Arc<NoticeDispatcher<TauriHost<R>, TauriStore<R>>>,
}

impl<R: Runtime> MeetingNoticeObserver<R> {
    /// The app's one observer, created on first use and kept in app state.
    pub fn new(app: &AppHandle<R>) -> Arc<Self> {
        if let Some(existing) = app.try_state::<Arc<Self>>() {
            return existing.inner().clone();
        }
        let observer = Arc::new(Self {
            app: app.clone(),
            dispatcher: Arc::new(NoticeDispatcher::new(
                TauriHost { app: app.clone() },
                TauriStore { app: app.clone() },
            )),
        });
        if app.manage(observer.clone()) {
            observer
        } else {
            app.state::<Arc<Self>>().inner().clone()
        }
    }
}

impl<R: Runtime> MeetingNoticeObserver<R> {
    /// For the browser extension attached to `capture`: claims the session's notice in the same
    /// durable store the app's own sender uses and returns the disclosure to post, or `None` when it
    /// must not post (setting off, capture not current, already claimed, storage failed).
    pub async fn claim_extension_notice(
        &self,
        capture: &CaptureIdentity,
        authority: &dyn CaptureAuthority,
    ) -> Option<String> {
        self.dispatcher
            .claim_for_extension(capture, authority)
            .await
    }

    /// Settles the extension's claim with the shared outcome of its notice. `reason` is a short
    /// code such as `chat_unavailable`, stored as evidence.
    pub async fn settle_extension_notice(
        &self,
        capture: &CaptureIdentity,
        outcome: NoticeOutcome,
        reason: &'static str,
    ) {
        self.dispatcher
            .settle_for_extension(capture, outcome, reason)
            .await;
    }
}

impl<R: Runtime> MeetingLifecycleObserver for MeetingNoticeObserver<R> {
    fn on_lifecycle(&self, event: &MeetingLifecycleEvent, authority: Arc<dyn CaptureAuthority>) {
        match event {
            MeetingLifecycleEvent::Started {
                capture,
                owner: LifecycleOwner::App,
                ..
            } => {
                let Some(work) = self.dispatcher.start(capture.clone(), authority) else {
                    return;
                };
                let app = self.app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(report) = work.await {
                        emit(&app, report);
                    }
                });
            }
            MeetingLifecycleEvent::ExtensionAttached { capture }
            | MeetingLifecycleEvent::Ended { capture, .. } => self.dispatcher.cancel(capture),
            _ => {}
        }
    }
}

fn emit<R: Runtime>(app: &AppHandle<R>, report: NoticeReport) {
    let outcome = match report.outcome {
        NoticeOutcome::Sent => MeetingNoticeOutcome::Sent,
        NoticeOutcome::NotSent => MeetingNoticeOutcome::NotSent,
        NoticeOutcome::CancelledBeforeDispatch => MeetingNoticeOutcome::CancelledBeforeDispatch,
        NoticeOutcome::Unconfirmed => MeetingNoticeOutcome::Unconfirmed,
    };
    let event = MeetingNoticeEvent {
        session_id: report.capture.session_id,
        capture_id: report.capture.capture_id,
        outcome,
        reason: report.reason.to_string(),
    };
    let _ = event.emit_to(
        app,
        EventTarget::AnyLabel {
            label: tauri_plugin_windows::AppWindow::Main.label(),
        },
    );
}

struct TauriHost<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> NoticeHost for TauriHost<R> {
    fn enabled(&self) -> bool {
        read_consent_auto_send_chat(&self.app).unwrap_or(false)
    }

    fn message(&self) -> String {
        MEETING_DISCLOSURE_MESSAGE.to_string()
    }

    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0)
    }

    async fn settle(&self) {
        tokio::time::sleep(SETTLE).await;
    }

    async fn post(
        &self,
        target: NoticeTarget,
        message: String,
        cancel: CancellationToken,
    ) -> PostResult {
        let answer = self
            .app
            .state::<AxWorker>()
            .run(move || {
                meeting_chat::post_notice(target.app, target.pid, &message, POST_TIMEOUT, &|| {
                    cancel.is_cancelled()
                })
            })
            .await;
        match answer {
            Ok(Ok(outcome)) => PostResult::Posted(outcome),
            Ok(Err(meeting_chat::Error::PermissionRequired)) => {
                PostResult::Refused("permission_required")
            }
            Ok(Err(error)) => {
                tracing::warn!(%error, "meeting_notice_post_refused");
                PostResult::Refused("chat_unavailable")
            }
            // The poster's thread stopped without answering: Return may have been pressed.
            Err(error) => {
                tracing::warn!(%error, "meeting_notice_post_lost");
                PostResult::Lost
            }
        }
    }

    async fn post_discovered(&self, message: String, wanted: StillWanted) -> DiscoveredPost {
        let unsent = DiscoveredPost {
            result: PostResult::Posted(meeting_chat::Outcome::NoCall),
            platform: "unknown",
        };
        let mut bundle_ids = match self.app.detect().list_mic_using_applications() {
            Ok(apps) => apps
                .into_iter()
                .map(|app| app.id.trim().to_string())
                .filter(|id| !id.is_empty())
                .collect::<Vec<_>>(),
            Err(error) => {
                tracing::warn!(%error, "meeting_notice_mic_apps_failed");
                return unsent;
            }
        };
        bundle_ids.sort();
        bundle_ids.dedup();
        let answer = self
            .app
            .state::<AxWorker>()
            .run(move || {
                // Queued behind other Accessibility work: the recording may have stopped meanwhile.
                wanted().then(|| anlg_detect::send_meeting_chat_message(message, bundle_ids))
            })
            .await;
        match answer {
            Ok(Some(sent)) => {
                if !sent.sent {
                    tracing::debug!(warnings = ?sent.warnings, "meeting_notice_discovery_unsent");
                }
                DiscoveredPost::of(&sent)
            }
            Ok(None) => DiscoveredPost {
                result: PostResult::Posted(meeting_chat::Outcome::Cancelled),
                platform: "unknown",
            },
            Err(error) => {
                tracing::warn!(%error, "meeting_notice_post_lost");
                DiscoveredPost {
                    result: PostResult::Lost,
                    platform: "unknown",
                }
            }
        }
    }

    async fn retry_wait(&self) {
        tokio::time::sleep(DISCOVERY_RETRY).await;
    }
}

fn read_consent_auto_send_chat<R: Runtime>(app: &AppHandle<R>) -> Option<bool> {
    use tauri_plugin_settings::SettingsPluginExt;

    let path = app.settings().settings_path().ok()?;
    let content = std::fs::read_to_string(path.as_str()).ok()?;
    let settings: serde_json::Value = serde_json::from_str(&content).ok()?;
    settings
        .get("general")?
        .get("consent_auto_send_chat")?
        .as_bool()
}

/// Claims in the app's database, written under the same barrier as other local writes.
struct TauriStore<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriStore<R> {
    fn database(&self) -> Result<tauri_plugin_db::ManagedState, String> {
        self.app
            .try_state::<tauri_plugin_db::ManagedState>()
            .map(|state| state.inner().clone())
            .ok_or_else(|| "database is not ready yet".to_string())
    }
}

impl<R: Runtime> NoticeStore for TauriStore<R> {
    async fn claim(&self, capture: &CaptureIdentity, now_ms: i64) -> Result<bool, String> {
        let database = self.database()?;
        let _guard = database.synced_write_guard().await;
        database.pool().claim(capture, now_ms).await
    }

    async fn settle(
        &self,
        capture: &CaptureIdentity,
        settlement: Settlement,
        now_ms: i64,
    ) -> Result<bool, String> {
        let database = self.database()?;
        let _guard = database.synced_write_guard().await;
        database.pool().settle(capture, settlement, now_ms).await
    }

    async fn release(&self, capture: &CaptureIdentity) -> Result<bool, String> {
        let database = self.database()?;
        let _guard = database.synced_write_guard().await;
        database.pool().release(capture).await
    }

    async fn record(&self, capture: &CaptureIdentity, delivery: Delivery) -> Result<(), String> {
        let database = self.database()?;
        let _guard = database.synced_write_guard().await;
        database.pool().record(capture, delivery).await
    }
}
