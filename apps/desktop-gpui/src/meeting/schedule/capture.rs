//! What capture does with each meeting chat sample and with the recording disclosure
//! (shipping's `stt/meeting-chat-capture.ts`, `stt/meeting-consent.ts` and
//! `stt/meeting-disclosure.ts`). The host runs the polling; these decide what is new.

use std::{collections::VecDeque, sync::LazyLock, time::Duration};

use anlg_db_execute::TransactionStatement;
use regex::Regex;
use serde_json::json;

use super::{
    apps::{ChatCapture, MeetingCapturedChatMessage, MeetingPlatform, MeetingSurface},
    records::{MAX_CHAT_RECORDS, platform_key},
};

pub const CHAT_INTERVAL: Duration = Duration::from_secs(5);
pub const SCREEN_INTERVAL: Duration = Duration::from_secs(3);
pub const DISCLOSURE_MESSAGE: &str =
    "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so";
pub const DISCLOSURE_VERSION: &str = "anarlog-disclosure-v1";
pub const DISCLOSURE_ATTEMPTS: usize = 30;
pub const DISCLOSURE_RETRY: Duration = Duration::from_secs(1);

pub const DISCLOSURE_FAILED: &str =
    "Recording started, but Anarlog could not post the meeting chat disclosure.";
pub const PARTICIPANT_DECLINED: &str =
    "A participant declined recording. Anarlog stopped listening.";
pub const SCREEN_PERMISSION: &str =
    "Shared screen capture needs Screen Recording permission in Settings";

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Shipping's 32-bit FNV pair over UTF-16 code units, so signatures match across apps.
fn text_hash(text: &str) -> String {
    let mut first: u32 = 0x811c9dc5;
    let mut second: u32 = 0x9e3779b9;
    for code in text.encode_utf16() {
        first = (first ^ u32::from(code)).wrapping_mul(0x01000193);
        second = (second ^ u32::from(code)).wrapping_mul(0x85ebca6b);
    }
    format!("{first:08x}{second:08x}")
}

fn surface_key(surface: &MeetingSurface) -> &'static str {
    match surface {
        MeetingSurface::Native => "native",
        MeetingSurface::Web => "web",
        MeetingSurface::Unknown => "unknown",
    }
}

/// Shipping's `getCapturedMeetingChatSignature`.
pub fn signature(context: &str, message: &MeetingCapturedChatMessage) -> String {
    let platform = platform_key(&message.platform);
    let surface = surface_key(&message.surface);
    if message.id.is_empty() {
        [
            context.to_owned(),
            platform,
            surface.into(),
            message.sender.clone().unwrap_or_default(),
            message.timestamp.clone().unwrap_or_default(),
            message.text.encode_utf16().count().to_string(),
            text_hash(&message.text),
        ]
        .join("\n")
    } else {
        [context, &platform, surface, &message.id].join("\n")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Warning {
    AccessibilityPermission,
    AccessibilityBus,
}

impl Warning {
    pub fn message(self) -> &'static str {
        match self {
            Self::AccessibilityPermission => {
                "Meeting chat capture needs Accessibility permission in Settings"
            }
            Self::AccessibilityBus => "Meeting chat capture needs the desktop accessibility bus",
        }
    }
}

/// One capture's new messages, each with its source signature.
#[derive(Debug, Default)]
pub struct Sample {
    pub entries: Vec<(MeetingCapturedChatMessage, String)>,
    pub warning: Option<Warning>,
}

/// Chat already seen in the meeting's current chat context. The first sample of a context is
/// a baseline: messages from before the recording are never filed into the note.
#[derive(Default)]
pub struct ChatState {
    baseline: Option<(String, String)>,
    seen: VecDeque<String>,
    last_warning: String,
}

impl ChatState {
    pub fn reset(&mut self) {
        self.baseline = None;
    }

    pub fn observe(&mut self, result: &ChatCapture) -> Sample {
        let mut sample = Sample::default();
        let warning = result.warnings.join("\n");
        if warning != self.last_warning
            && (warning.contains("accessibility permission")
                || warning.contains("accessibility bus"))
        {
            sample.warning = Some(if warning.contains("accessibility bus") {
                Warning::AccessibilityBus
            } else {
                Warning::AccessibilityPermission
            });
        }
        self.last_warning = warning;
        let context = result
            .context_id
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        let Some(app) = result.app.as_ref().filter(|_| !context.is_empty()) else {
            return sample;
        };
        let excluded = normalize(DISCLOSURE_MESSAGE);
        let messages: Vec<&MeetingCapturedChatMessage> = result
            .messages
            .iter()
            .filter(|message| normalize(&message.text) != excluded)
            .collect();
        let messages = &messages[messages.len().saturating_sub(MAX_CHAT_RECORDS)..];
        let key = (app.id.clone(), context.to_owned());
        if self.baseline.as_ref() != Some(&key) {
            self.baseline = Some(key);
            self.seen.clear();
            for message in messages {
                self.remember(signature(context, message));
            }
            return sample;
        }
        for message in messages {
            let signature = signature(context, message);
            if !self.seen.contains(&signature)
                && !sample.entries.iter().any(|(_, seen)| seen == &signature)
            {
                sample.entries.push(((*message).clone(), signature));
            }
        }
        sample
    }

    pub fn remember(&mut self, signature: String) {
        self.seen.retain(|seen| seen != &signature);
        self.seen.push_back(signature);
        while self.seen.len() > MAX_CHAT_RECORDS {
            self.seen.pop_front();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consent {
    Consented,
    Declined,
}

impl Consent {
    fn key(self) -> &'static str {
        match self {
            Self::Consented => "consented",
            Self::Declined => "declined",
        }
    }
}

static DECLINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((i\s+)?(do\s+not|don't|does\s+not|doesn't)\s+consent|stop\s+recording)\b")
        .expect("decline pattern")
});
static CONSENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(i\s+consent|i\s+agree\s+to\s+(being\s+)?record)").expect("consent pattern")
});

/// Shipping's `interpretChatAsConsentResponse`.
pub fn consent(text: &str) -> Option<Consent> {
    let text = normalize(text);
    if text.is_empty() || text == normalize(DISCLOSURE_MESSAGE) {
        None
    } else if DECLINE.is_match(&text) {
        Some(Consent::Declined)
    } else if CONSENT.is_match(&text) {
        Some(Consent::Consented)
    } else {
        None
    }
}

pub fn consent_statement(
    session: &str,
    sender: Option<&str>,
    consent: Consent,
    at: &str,
) -> TransactionStatement {
    let participant = sender.map(str::trim).filter(|sender| !sender.is_empty());
    TransactionStatement {
        sql: "INSERT INTO session_participant_consent (session_id, participant_key, status, source, updated_at)
            VALUES (?, ?, ?, 'explicit_chat_reply', ?)
            ON CONFLICT(session_id, participant_key) DO UPDATE SET
              status = excluded.status, source = excluded.source, updated_at = excluded.updated_at"
            .into(),
        params: vec![
            json!(session),
            json!(participant.unwrap_or("unidentified")),
            json!(consent.key()),
            json!(at),
        ],
        expected_rows_affected: None,
    }
}

/// Shipping's `disclosurePlatformFromSend` and `disclosureSurfaceFromSend`.
pub fn disclosure_target(
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
) -> (&'static str, &'static str) {
    let platform = match platform {
        MeetingPlatform::Zoom => "zoom",
        MeetingPlatform::GoogleMeet => "google_meet",
        MeetingPlatform::MicrosoftTeams => "teams",
        MeetingPlatform::Slack => "slack_huddle",
        MeetingPlatform::Webex => "webex",
        MeetingPlatform::Discord | MeetingPlatform::Unknown => "unknown",
    };
    let surface = match (platform, surface) {
        ("slack_huddle", MeetingSurface::Native) => "huddle",
        (_, surface) => surface_key(surface),
    };
    (platform, surface)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    Sent,
    NotSent,
    Cancelled,
}

pub fn disclosure_statement(
    session: &str,
    at: &str,
    target: (&str, &str),
    delivery: Delivery,
    failure: &str,
) -> TransactionStatement {
    TransactionStatement {
        sql: "INSERT INTO session_disclosure_attempts (id, session_id, attempted_at, platform, surface,
                message_version, message, delivery, failure_reason) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
            .into(),
        params: vec![
            json!(uuid::Uuid::new_v4().to_string()),
            json!(session),
            json!(at),
            json!(target.0),
            json!(target.1),
            json!(DISCLOSURE_VERSION),
            json!(DISCLOSURE_MESSAGE),
            json!(match delivery {
                Delivery::Sent => "sent",
                Delivery::NotSent => "not_sent",
                Delivery::Cancelled => "cancelled",
            }),
            json!(failure),
        ],
        expected_rows_affected: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::schedule::apps::{MeetingApp, MeetingChatDirection};

    fn message(id: &str, text: &str) -> MeetingCapturedChatMessage {
        MeetingCapturedChatMessage {
            id: id.into(),
            platform: MeetingPlatform::Zoom,
            surface: MeetingSurface::Native,
            sender: Some("Mina".into()),
            timestamp: None,
            direction: Some(MeetingChatDirection::Incoming),
            text: text.into(),
            links: vec![],
        }
    }

    fn capture(context: &str, messages: Vec<MeetingCapturedChatMessage>) -> ChatCapture {
        ChatCapture {
            app: Some(MeetingApp {
                id: "us.zoom.xos".into(),
                name: "zoom.us".into(),
            }),
            platform: MeetingPlatform::Zoom,
            surface: MeetingSurface::Native,
            context_id: Some(context.into()),
            messages,
            warnings: vec![],
        }
    }

    #[test]
    fn chat_from_before_the_recording_and_the_disclosure_itself_are_never_captured() {
        let mut state = ChatState::default();
        let before = capture("call-1", vec![message("1", "hello")]);
        assert!(state.observe(&before).entries.is_empty());
        let next = capture(
            "call-1",
            vec![
                message("1", "hello"),
                message("2", DISCLOSURE_MESSAGE),
                message("3", "agenda link"),
            ],
        );
        let sample = state.observe(&next);
        assert_eq!(sample.entries.len(), 1);
        assert_eq!(sample.entries[0].0.id, "3");
        // A new chat context is a new baseline.
        assert!(
            state
                .observe(&capture("call-2", vec![message("4", "hi")]))
                .entries
                .is_empty()
        );
    }

    #[test]
    fn untitled_messages_sign_like_shipping_and_replies_read_as_consent() {
        let unnamed = message("", "Ünïcode 👋");
        // Values computed by shipping's JavaScript.
        assert_eq!(
            signature("ctx", &unnamed),
            "ctx\nzoom\nnative\nMina\n\n10\n3e5c23276f9664c3"
        );
        assert_eq!(
            super::super::records::source_hash(&signature("ctx", &message("1", "x"))),
            "76de75697e0671b9e08871a06fec36da"
        );
        assert_eq!(consent("I do not consent"), Some(Consent::Declined));
        assert_eq!(consent("please STOP recording"), Some(Consent::Declined));
        assert_eq!(
            consent("I agree to being recorded"),
            Some(Consent::Consented)
        );
        assert_eq!(consent(DISCLOSURE_MESSAGE), None);
        assert_eq!(consent("sounds good"), None);
    }
}
