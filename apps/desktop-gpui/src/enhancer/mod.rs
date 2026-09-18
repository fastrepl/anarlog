//! `services/enhancer` and the `enhance` / `title` AI tasks: turning a
//! session's transcript, memo, and template into a summary through the
//! configured language model, then persisting it like the frontend does.
//!
//! This module holds the shell-neutral logic; `db.rs` owns the SQL and
//! `workspace/enhance.rs` drives the task lifecycle and the UI.

pub mod eligibility;
pub mod images;
pub mod prompts;
pub mod runner;
pub mod summary_length;
pub mod text;
pub mod validator;

pub use eligibility::{Eligibility, SkipCode, eligibility};

/// `SessionContentSnapshot["enhancedNotes"][number]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnhancedNote {
    pub id: String,
    pub title: String,
    /// The stored body.
    pub content: String,
    pub content_format: String,
    pub template_id: String,
    pub position: i64,
}

/// A transcript row the enhancer reads: its words' texts plus the memo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotTranscript {
    pub id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub memo: String,
    pub words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotParticipant {
    pub human_id: String,
    pub name: String,
    pub job_title: String,
}

/// A rendered transcript segment (`renderTranscriptSegments` output).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentPayload {
    pub speaker_label: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// `SessionContentSnapshot` plus the rendered segments and the supplemental
/// context (`formatSessionSourceAppsContext`, `formatMeetingChatContext`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub session_id: String,
    pub owner_user_id: String,
    pub title: String,
    pub created_at: String,
    pub event_id: String,
    pub event_json: String,
    /// `formatMeetingChatRecordsAsMarkdown` over the meeting chat records.
    pub meeting_chat: String,
    /// `rawNoteId`: the memo document's id when one exists.
    pub raw_note_id: Option<String>,
    pub raw_template_id: String,
    pub raw_content: String,
    pub raw_content_format: String,
    pub raw_markdown: String,
    pub enhanced_notes: Vec<EnhancedNote>,
    pub transcripts: Vec<SnapshotTranscript>,
    pub participants: Vec<SnapshotParticipant>,
    pub segments: Vec<SegmentPayload>,
    pub supplemental_context: String,
}

impl Snapshot {
    pub fn word_lists(&self) -> Vec<Vec<String>> {
        self.transcripts
            .iter()
            .map(|transcript| transcript.words.clone())
            .collect()
    }

    pub fn eligibility(&self) -> Eligibility {
        eligibility(&self.word_lists())
    }

    pub fn enhanced_note(&self, note_id: &str) -> Option<&EnhancedNote> {
        self.enhanced_notes.iter().find(|note| note.id == note_id)
    }

    /// `getMatchingEnhancedNote`.
    pub fn matching_enhanced_note(&self, template_id: Option<&str>) -> Option<&EnhancedNote> {
        let template_id = template_id.unwrap_or("");
        self.enhanced_notes
            .iter()
            .find(|note| note.template_id == template_id)
    }

    /// `getAutoEnhancedNote`: the template's note, else the first by position.
    pub fn auto_enhanced_note(&self, template_id: Option<&str>) -> Option<&EnhancedNote> {
        self.matching_enhanced_note(template_id).or_else(|| {
            self.enhanced_notes.iter().min_by(|left, right| {
                left.position
                    .cmp(&right.position)
                    .then_with(|| left.id.cmp(&right.id))
            })
        })
    }
}

/// `PendingAutoEnhanceJob`: the durable marker in `app_settings` that lets
/// an interrupted auto-summary resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingJob {
    pub session_id: String,
    pub note_id: String,
    pub template_id: String,
    pub expected_body: String,
    pub expected_content_format: String,
    pub generation: String,
}

pub const PENDING_AUTO_ENHANCE_SETTING_PREFIX: &str = "auto_enhance_pending:";

/// `resolveTemplateId`: an explicit `None` request means "Auto"; otherwise
/// the request, the memo's template, then the selected default template.
pub fn resolve_template_id(
    requested: Option<Option<&str>>,
    memo_template_id: &str,
    selected_template_id: Option<&str>,
) -> Option<String> {
    match requested {
        Some(None) => None,
        Some(Some(id)) if !id.is_empty() => Some(id.to_string()),
        _ => {
            if !memo_template_id.is_empty() {
                Some(memo_template_id.to_string())
            } else {
                selected_template_id
                    .filter(|id| !id.is_empty())
                    .map(str::to_string)
            }
        }
    }
}

/// `formatMeetingChatContext` over the `meeting_chat` documents' bodies.
pub fn meeting_chat_context(bodies: &[String]) -> String {
    let markdown = meeting_chat_markdown(bodies);
    if markdown.is_empty() {
        String::new()
    } else {
        format!("## Meeting chat\n{markdown}")
    }
}

/// `formatMeetingChatRecordsAsMarkdown` over the stored `meeting_chat`
/// document bodies.
pub fn meeting_chat_markdown(bodies: &[String]) -> String {
    let lines: Vec<String> = bodies
        .iter()
        .filter(|body| body.len() <= 16 * 1024)
        .filter_map(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        .filter_map(|value| {
            let platform = value.get("platform")?.as_str()?;
            let platform = match platform {
                "zoom" => "Zoom",
                "googleMeet" => "Google Meet",
                "microsoftTeams" => "Microsoft Teams",
                "slack" => "Slack",
                "discord" => "Discord",
                "webex" => "Webex",
                "unknown" => "Meeting app",
                _ => return None,
            };
            let surface = value.get("surface")?.as_str()?;
            if !matches!(surface, "native" | "web" | "unknown") {
                return None;
            }
            value.get("id")?.as_str()?;
            value.get("links")?.as_array()?;
            let text = value.get("text")?.as_str()?;
            let direction = match value.get("direction").and_then(|d| d.as_str()) {
                Some("outgoing") => Some("sent"),
                Some("incoming") => Some("received"),
                _ => None,
            };
            let metadata: Vec<&str> = [
                Some(platform),
                value.get("timestamp").and_then(|t| t.as_str()),
                value.get("sender").and_then(|s| s.as_str()),
                direction,
            ]
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
            .collect();
            Some(format!(
                "- {}\n  {}",
                metadata.join(" · "),
                text.replace('\n', "\n  ")
            ))
        })
        .collect();
    lines.join("\n")
}

/// `formatSessionSourceAppsContext` over `sessions.source_apps_json`.
pub fn source_apps_context(source_apps_json: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source_apps_json) else {
        return String::new();
    };
    let mut platforms: Vec<String> = Vec::new();
    for source in value.as_array().into_iter().flatten() {
        if let Some(platform) = source
            .get("platform")
            .and_then(|p| p.as_str())
            .map(str::trim)
            .filter(|p| !p.is_empty())
            && !platforms.iter().any(|existing| existing == platform)
        {
            platforms.push(platform.to_string());
        }
    }
    if platforms.is_empty() {
        String::new()
    } else {
        format!("Meeting platform: {}", platforms.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_resolution_prefers_explicit_then_memo_then_default() {
        assert_eq!(resolve_template_id(Some(None), "memo", Some("sel")), None);
        assert_eq!(
            resolve_template_id(Some(Some("req")), "memo", Some("sel")),
            Some("req".into())
        );
        assert_eq!(
            resolve_template_id(None, "memo", Some("sel")),
            Some("memo".into())
        );
        assert_eq!(
            resolve_template_id(None, "", Some("sel")),
            Some("sel".into())
        );
        assert_eq!(resolve_template_id(None, "", None), None);
        assert_eq!(resolve_template_id(Some(Some("")), "", None), None);
    }

    #[test]
    fn supplemental_context_matches_the_frontend_format() {
        assert_eq!(
            source_apps_context(
                r#"[{"platform":"Zoom"},{"platform":"Zoom"},{"platform":" Teams "}]"#
            ),
            "Meeting platform: Zoom, Teams"
        );
        assert_eq!(source_apps_context("[]"), "");
        let chat = meeting_chat_context(&[
            r#"{"id":"1","platform":"zoom","surface":"web","sender":"Ada","timestamp":"10:00","direction":"incoming","text":"hi\nthere","links":[]}"#.to_string(),
            r#"{"id":"2","platform":"nope","surface":"web","text":"x","links":[]}"#.to_string(),
        ]);
        assert_eq!(
            chat,
            "## Meeting chat\n- Zoom · 10:00 · Ada · received\n  hi\n  there"
        );
        assert_eq!(meeting_chat_context(&[]), "");
    }
}
