//! OS notifications through the shared `notification` crate (the plugin's
//! `show_notification` without the Tauri wrapper): the batch-completed and
//! summary-ready cards, and the click that opens their session.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anlg_notification::{Notification, NotificationContext, NotificationSource};

/// `BATCH_COMPLETED_NOTIFICATION_KEY_PREFIX`
pub const BATCH_COMPLETED_KEY_PREFIX: &str = "batch-completed:";
/// `SUMMARY_READY_NOTIFICATION_KEY_PREFIX`
pub const SUMMARY_READY_KEY_PREFIX: &str = "summary-ready:";

/// `BATCH_COMPLETED_NOTIFICATION_TIMEOUT_SECONDS`
const BATCH_COMPLETED_TIMEOUT: Duration = Duration::from_secs(15);
/// `SUMMARY_READY_NOTIFICATION_TIMEOUT_SECONDS`
const SUMMARY_READY_TIMEOUT: Duration = Duration::from_secs(15);

/// A notification the user acted on (`notification_accept` /
/// `notification_confirm`), resolved to the session it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub session_id: String,
}

pub struct Notifications {
    clicks: Arc<Mutex<Receiver<Opened>>>,
}

impl gpui::Global for Notifications {}

impl Notifications {
    /// Register the accept / confirm handlers once.
    pub fn install() -> Self {
        let (sender, clicks) = std::sync::mpsc::channel::<Opened>();
        let accept = sender.clone();
        anlg_notification::setup_expanded_accept_handler(move |context| {
            forward(&accept, context);
        });
        anlg_notification::setup_collapsed_confirm_handler(move |context| {
            forward(&sender, context);
        });
        Self {
            clicks: Arc::new(Mutex::new(clicks)),
        }
    }

    /// The clicks since the last poll.
    pub fn take_opened(&self) -> Vec<Opened> {
        let Ok(receiver) = self.clicks.lock() else {
            return Vec::new();
        };
        let mut opened = Vec::new();
        while let Ok(click) = receiver.try_recv() {
            opened.push(click);
        }
        opened
    }
}

fn forward(sender: &Sender<Opened>, context: NotificationContext) {
    if let Some(session_id) = session_of(&context) {
        let _ = sender.send(Opened { session_id });
    }
}

/// The session a notification opens: its `session` source, else the id in a
/// `batch-completed:` / `summary-ready:` key (`parseBatchCompletedNotificationKey`).
pub fn session_of(context: &NotificationContext) -> Option<String> {
    if let Some(NotificationSource::Session { session_id }) = &context.source {
        return Some(session_id.clone());
    }
    session_from_key(&context.key)
}

pub fn session_from_key(key: &str) -> Option<String> {
    let value = key
        .strip_prefix(BATCH_COMPLETED_KEY_PREFIX)
        .or_else(|| key.strip_prefix(SUMMARY_READY_KEY_PREFIX))?;
    let session_id = match value.rfind(':') {
        Some(index) => &value[..index],
        None => value,
    };
    (!session_id.is_empty()).then(|| session_id.to_string())
}

fn session_notification(
    key_prefix: &str,
    session_id: &str,
    title: &str,
    message: String,
    timeout: Duration,
) -> Notification {
    Notification {
        key: Some(format!("{key_prefix}{session_id}:{}", uuid::Uuid::new_v4())),
        title: title.to_string(),
        message,
        timeout: Some(timeout),
        source: Some(NotificationSource::Session {
            session_id: session_id.to_string(),
        }),
        start_time: None,
        participants: None,
        event_details: None,
        action_label: Some("Open Anarlog".to_string()),
        action_variant: None,
        options: None,
        footer: None,
        icon: None,
    }
}

/// `showBatchCompletedNotification`'s card.
pub fn batch_completed(session_id: &str) -> Notification {
    session_notification(
        BATCH_COMPLETED_KEY_PREFIX,
        session_id,
        "Transcription complete",
        "Your transcript is ready.".to_string(),
        BATCH_COMPLETED_TIMEOUT,
    )
}

/// `showSummaryReadyNotification`'s card.
pub fn summary_ready(session_id: &str, session_title: Option<&str>) -> Notification {
    let title = session_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    session_notification(
        SUMMARY_READY_KEY_PREFIX,
        session_id,
        "Summary ready",
        match title {
            Some(title) => format!("\"{title}\" is ready to read."),
            None => "Your summary is ready.".to_string(),
        },
        SUMMARY_READY_TIMEOUT,
    )
}

/// Show a card; on Linux the windows are GTK, so the shared main loop has to
/// be running first.
pub fn show(notification: &Notification) {
    #[cfg(target_os = "linux")]
    if !crate::gtk_loop::ensure_running() {
        tracing::warn!("gtk init failed; notification skipped");
        return;
    }
    anlg_notification::show(notification);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_carry_the_session_and_copy() {
        let card = batch_completed("s1");
        assert!(
            card.key
                .as_deref()
                .unwrap()
                .starts_with("batch-completed:s1:")
        );
        assert_eq!(card.title, "Transcription complete");
        assert_eq!(card.message, "Your transcript is ready.");
        assert_eq!(card.action_label.as_deref(), Some("Open Anarlog"));
        assert_eq!(card.timeout, Some(Duration::from_secs(15)));
        assert!(matches!(
            card.source,
            Some(NotificationSource::Session { ref session_id }) if session_id == "s1"
        ));

        let card = summary_ready("s2", Some("  Weekly sync "));
        assert_eq!(card.message, "\"Weekly sync\" is ready to read.");
        assert_eq!(
            summary_ready("s2", Some("  ")).message,
            "Your summary is ready."
        );
        assert_eq!(summary_ready("s2", None).title, "Summary ready");
    }

    #[test]
    fn clicks_resolve_their_session() {
        let context = NotificationContext {
            key: "batch-completed:s1:abc".into(),
            source: None,
        };
        assert_eq!(session_of(&context).as_deref(), Some("s1"));
        assert_eq!(
            session_from_key("summary-ready:with:colons:uuid").as_deref(),
            Some("with:colons")
        );
        assert_eq!(session_from_key("other:s1"), None);
        let context = NotificationContext {
            key: "x".into(),
            source: Some(NotificationSource::Session {
                session_id: "s9".into(),
            }),
        };
        assert_eq!(session_of(&context).as_deref(), Some("s9"));
    }
}
