//! Captured meeting chat and shared screens, stored as `session_documents` of kind
//! `meeting_chat` and `meeting_screen` (shipping's `stt/meeting-chat-records.ts` and
//! `stt/meeting-screen-records.ts`).

use anlg_db_execute::TransactionStatement;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::apps::{
    MeetingCapturedChatMessage, MeetingChatDirection, MeetingPlatform, MeetingSurface,
};

pub const MAX_CHAT_RECORDS: usize = 1000;
pub const MAX_CHAT_RECORD_BYTES: usize = 16 * 1024;

pub const CHAT_SQL: &str = "SELECT id, body, created_at FROM (
        SELECT id, body, created_at, sort_order FROM session_documents
        WHERE session_id = ? AND kind = 'meeting_chat' AND deleted_at IS NULL
          AND length(CAST(body AS BLOB)) <= 16384
        ORDER BY sort_order DESC, created_at DESC, id DESC LIMIT 1000)
    ORDER BY sort_order, created_at, id";

pub const SCREENS_SQL: &str = "SELECT id, body FROM session_documents
    WHERE session_id = ? AND kind = 'meeting_screen' AND deleted_at IS NULL
    ORDER BY sort_order ASC, created_at ASC";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRecord {
    pub id: String,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub sender: Option<String>,
    pub timestamp: Option<String>,
    pub direction: Option<MeetingChatDirection>,
    pub text: String,
    pub links: Vec<String>,
    pub captured_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRecord {
    #[serde(default)]
    pub id: String,
    pub attachment_id: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    pub captured_at: String,
}

/// Shipping's `formatMeetingPlatform`.
pub fn platform_label(platform: &MeetingPlatform) -> &'static str {
    match platform {
        MeetingPlatform::Zoom => "Zoom",
        MeetingPlatform::GoogleMeet => "Google Meet",
        MeetingPlatform::MicrosoftTeams => "Microsoft Teams",
        MeetingPlatform::Slack => "Slack",
        MeetingPlatform::Discord => "Discord",
        MeetingPlatform::Webex => "Webex",
        MeetingPlatform::Unknown => "Meeting app",
    }
}

/// The camelCase name stored in records, e.g. `googleMeet`.
pub fn platform_key(platform: &MeetingPlatform) -> String {
    serde_json::to_value(platform)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

pub fn platform_from_key(key: &str) -> Option<MeetingPlatform> {
    serde_json::from_value(Value::String(key.into())).ok()
}

/// Shipping's 64-bit FNV pair over the source signature, as hex.
pub fn source_hash(value: &str) -> String {
    let mut first: u64 = 0xcbf29ce484222325;
    let mut second: u64 = 0x84222325cbf29ce4;
    for byte in value.bytes() {
        first = (first ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        second = (second ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("{first:016x}{second:016x}")
}

/// The chat inserts for newly seen messages; oversized messages are skipped.
pub fn chat_statements(
    session: &str,
    entries: &[(MeetingCapturedChatMessage, String)],
    captured_at: &str,
    captured_ms: i64,
) -> Vec<TransactionStatement> {
    let start = entries.len().saturating_sub(MAX_CHAT_RECORDS);
    entries[start..]
        .iter()
        .enumerate()
        .filter_map(|(index, (message, signature))| {
            let record = ChatRecord {
                id: message.id.clone(),
                platform: message.platform.clone(),
                surface: message.surface.clone(),
                sender: message.sender.clone(),
                timestamp: message.timestamp.clone(),
                direction: message.direction.clone(),
                text: message.text.clone(),
                links: message.links.clone(),
                captured_at: captured_at.into(),
            };
            let body = serde_json::to_string(&record).ok()?;
            if body.len() > MAX_CHAT_RECORD_BYTES {
                tracing::warn!("skipped oversized meeting chat message");
                return None;
            }
            let hash = source_hash(signature);
            Some(TransactionStatement {
                sql: "INSERT INTO session_documents (id, session_id, kind, title, body_format, body,
                        source_hash, generation_metadata_json, sort_order, created_by, updated_by,
                        created_at, updated_at, deleted_at)
                    SELECT ?, id, 'meeting_chat', ?, 'json', ?, ?, ?, ?, owner_user_id, owner_user_id, ?, ?, NULL
                    FROM sessions WHERE id = ? AND deleted_at IS NULL
                    ON CONFLICT(id) DO NOTHING"
                    .into(),
                params: vec![
                    json!(format!("{session}:meeting-chat:{hash}")),
                    json!(format!("{} chat", platform_label(&message.platform))),
                    json!(body),
                    json!(hash),
                    json!(json!({"source": "meeting_ax", "version": 1}).to_string()),
                    json!(captured_ms * 100 + index as i64),
                    json!(captured_at),
                    json!(captured_at),
                    json!(session),
                ],
                expected_rows_affected: None,
            })
        })
        .collect()
}

pub fn screen_statement(
    session: &str,
    attachment_id: &str,
    platform: Option<&MeetingPlatform>,
    width: u32,
    height: u32,
    captured_at: &str,
    captured_ms: i64,
) -> TransactionStatement {
    let id = format!("{session}:meeting-screen:{attachment_id}");
    let record = ScreenRecord {
        id: id.clone(),
        attachment_id: attachment_id.into(),
        platform: platform.map(platform_key),
        width,
        height,
        captured_at: captured_at.into(),
    };
    let title = platform.map_or("Shared screen".to_owned(), |platform| {
        format!("{} shared screen", platform_label(platform))
    });
    TransactionStatement {
        sql: "INSERT INTO session_documents (id, session_id, kind, title, body_format, body,
                source_hash, generation_metadata_json, sort_order, created_by, updated_by,
                created_at, updated_at, deleted_at)
            SELECT ?, id, 'meeting_screen', ?, 'json', ?, ?, ?, ?, owner_user_id, owner_user_id, ?, ?, NULL
            FROM sessions WHERE id = ? AND deleted_at IS NULL
            ON CONFLICT(id) DO NOTHING"
            .into(),
        params: vec![
            json!(id),
            json!(title),
            json!(serde_json::to_string(&record).unwrap_or_default()),
            json!(attachment_id),
            json!(json!({"source": "meeting_screen_share", "version": 1}).to_string()),
            json!(captured_ms),
            json!(captured_at),
            json!(captured_at),
            json!(session),
        ],
        expected_rows_affected: None,
    }
}

/// Shipping's `parseMeetingChatRows`: unreadable rows are skipped, only http(s) links stay.
pub fn parse_chat(rows: &[Value]) -> Vec<ChatRecord> {
    let start = rows.len().saturating_sub(MAX_CHAT_RECORDS);
    rows[start..]
        .iter()
        .filter_map(|row| {
            let body = row["body"].as_str()?;
            if body.len() > MAX_CHAT_RECORD_BYTES {
                return None;
            }
            let value: Value = serde_json::from_str(body).ok()?;
            let platform = platform_from_key(value["platform"].as_str()?)?;
            let surface = serde_json::from_value(value["surface"].clone()).ok()?;
            let text = |key: &str| value[key].as_str().map(str::to_owned);
            Some(ChatRecord {
                id: text("id")?,
                platform,
                surface,
                sender: text("sender"),
                timestamp: text("timestamp"),
                direction: serde_json::from_value(value["direction"].clone()).ok(),
                text: text("text")?,
                links: value["links"]
                    .as_array()?
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|link| link.starts_with("http://") || link.starts_with("https://"))
                    .map(str::to_owned)
                    .collect(),
                captured_at: text("capturedAt")
                    .or_else(|| row["created_at"].as_str().map(str::to_owned))
                    .unwrap_or_default(),
            })
        })
        .collect()
}

pub fn parse_screens(rows: &[Value]) -> Vec<ScreenRecord> {
    rows.iter()
        .filter_map(|row| {
            let mut record: ScreenRecord = serde_json::from_str(row["body"].as_str()?).ok()?;
            record.id = row["id"].as_str()?.to_owned();
            Some(record)
        })
        .collect()
}

/// Shipping's `formatMeetingChatRecordsAsMarkdown`, the AI context's meeting chat.
pub fn chat_markdown(records: &[ChatRecord]) -> String {
    let lines: Vec<String> = records
        .iter()
        .map(|record| {
            let direction = match record.direction {
                Some(MeetingChatDirection::Outgoing) => Some("sent"),
                Some(MeetingChatDirection::Incoming) => Some("received"),
                None => None,
            };
            let metadata: Vec<&str> = [
                Some(platform_label(&record.platform)),
                record.timestamp.as_deref(),
                record.sender.as_deref(),
                direction,
            ]
            .into_iter()
            .flatten()
            .filter(|value| !value.is_empty())
            .collect();
            format!(
                "- {}\n  {}",
                metadata.join(" · "),
                record.text.replace('\n', "\n  ")
            )
        })
        .collect();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_chat_rows_round_trip_and_drop_unsafe_links() {
        let message = MeetingCapturedChatMessage {
            id: "m1".into(),
            platform: MeetingPlatform::GoogleMeet,
            surface: MeetingSurface::Web,
            sender: Some("Mina".into()),
            timestamp: Some("10:02".into()),
            direction: Some(MeetingChatDirection::Incoming),
            text: "Deck: https://example.com/deck".into(),
            links: vec![
                "https://example.com/deck".into(),
                "javascript:alert(1)".into(),
            ],
        };
        let statements = chat_statements(
            "s-1",
            &[(message, "sig".into())],
            "2026-11-17T10:02:00.000Z",
            1,
        );
        assert_eq!(statements.len(), 1);
        let body = statements[0].params[2].as_str().unwrap();
        assert!(body.contains("\"platform\":\"googleMeet\""));
        let records = parse_chat(&[json!({"id": "d", "body": body, "created_at": ""})]);
        assert_eq!(records[0].links, vec!["https://example.com/deck"]);
        assert_eq!(
            chat_markdown(&records),
            "- Google Meet · 10:02 · Mina · received\n  Deck: https://example.com/deck"
        );
    }
}
