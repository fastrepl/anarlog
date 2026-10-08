use crate::{Event, Participant, Transcript, common_derives};
#[allow(unused_imports)]
use anlg_askama_utils::filters;

common_derives! {
    pub struct SessionContext {
        pub session_id: Option<String>,
        pub title: Option<String>,
        pub date: Option<String>,
        pub raw_content: Option<String>,
        pub enhanced_content: Option<String>,
        pub meeting_chat: Option<String>,
        pub transcript: Option<Transcript>,
        pub participants: Vec<Participant>,
        pub event: Option<Event>,
    }
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "chat.system.md.jinja")]
    pub struct ChatSystem {
        pub language: Option<String>,
    }
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "context.block.md.jinja")]
    pub struct ContextBlock {
        pub contexts: Vec<SessionContext>,
        pub current_session_id: Option<String>,
    }
}

impl ContextBlock {
    fn is_current(&self, ctx: &SessionContext) -> bool {
        ctx.session_id.is_some() && ctx.session_id == self.current_session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_askama_utils::tpl_snapshot_with_assert;

    tpl_snapshot_with_assert!(
        test_context_block_wrapped,
        ContextBlock {
            contexts: vec![SessionContext {
                session_id: None,
                title: Some("Q1 Planning".to_string()),
                date: Some("2025-03-01".to_string()),
                raw_content: None,
                enhanced_content: Some("Summary of Q1 goals.".to_string()),
                meeting_chat: Some(
                    "- Slack · 10:42 AM · Ada · received\n  Review the rollout plan."
                        .to_string(),
                ),
                transcript: None,
                participants: vec![],
                event: None,
            }],
            current_session_id: None,
        },
        |v| v.starts_with("<context>") && v.trim_end().ends_with("</context>"),
        @r#"
    <context>

    Title: Q1 Planning

    Date: 2025-03-01

    Enhanced Meeting Summary:
    Summary of Q1 goals.

    Meeting Chat:
    - Slack · 10:42 AM · Ada · received
      Review the rollout plan.
    </context>
    "#);

    tpl_snapshot_with_assert!(
        test_context_block_marks_current_session,
        ContextBlock {
            contexts: vec![
                SessionContext {
                    session_id: Some("session-old".to_string()),
                    title: Some("Old Meeting".to_string()),
                    date: None,
                    raw_content: Some("Old notes".to_string()),
                    enhanced_content: None,
                    meeting_chat: None,
                    transcript: None,
                    participants: vec![],
                    event: None,
                },
                SessionContext {
                    session_id: Some("session-new".to_string()),
                    title: Some("New Meeting".to_string()),
                    date: None,
                    raw_content: Some("New notes".to_string()),
                    enhanced_content: None,
                    meeting_chat: None,
                    transcript: None,
                    participants: vec![],
                    event: None,
                },
            ],
            current_session_id: Some("session-new".to_string()),
        },
        |v| v.contains("Session ID: session-new\nCurrent note:") && !v.contains("Session ID: session-old\nCurrent note:"),
        @r#"
    <context>

    Session ID: session-old

    Title: Old Meeting

    User written note:
    Old notes

    ---

    Session ID: session-new
    Current note: the user is viewing this note right now.

    Title: New Meeting

    User written note:
    New notes
    </context>
    "#);

    tpl_snapshot_with_assert!(
        test_context_block_transcript_timestamps_for_recent_recaps,
        ContextBlock {
            contexts: vec![SessionContext {
                session_id: None,
                title: Some("Standup".to_string()),
                date: None,
                raw_content: None,
                enhanced_content: None,
                meeting_chat: None,
                transcript: Some(Transcript {
                    segments: vec![
                        crate::Segment {
                            text: "Kickoff".to_string(),
                            speaker: "Alice".to_string(),
                            start_ms: Some(5_000),
                        },
                        crate::Segment {
                            text: "Status update".to_string(),
                            speaker: "Bob".to_string(),
                            start_ms: Some(3_725_000),
                        },
                        crate::Segment {
                            text: "Imported line".to_string(),
                            speaker: "Carol".to_string(),
                            start_ms: None,
                        },
                    ],
                    started_at: None,
                    ended_at: None,
                }),
                participants: vec![],
                event: None,
            }],
            current_session_id: None,
        },
        |v| v.contains("[00:05] Alice: Kickoff") && v.contains("[62:05] Bob: Status update") && v.contains("\nCarol: Imported line"),
        @r#"
    <context>

    Title: Standup

    Full Meeting Transcript:

    [00:05] Alice: Kickoff
    [62:05] Bob: Status update
    Carol: Imported line
    </context>
    "#);
}
