use crate::{Event, Participant, Transcript, common_derives};
#[allow(unused_imports)]
use hypr_askama_utils::filters;

common_derives! {
    pub struct SessionContext {
        pub title: Option<String>,
        pub date: Option<String>,
        pub raw_content: Option<String>,
        pub enhanced_content: Option<String>,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_askama_utils::tpl_snapshot_with_assert;

    tpl_snapshot_with_assert!(
        test_chat_system,
        ChatSystem {
            language: None,
        },
        |v| !v.contains("Context:"),
        fixed_date = "2025-01-01",
        @r#"
    # General Instructions

    Current date: 2025-01-01

    - You are a helpful AI meeting assistant in Hyprnote, an intelligent meeting platform that transcribes and analyzes meetings. Your purpose is to help users understand their meeting content better.
    - Always respond in English, unless the user explicitly asks for a different language.
    - Always keep your responses concise, professional, and directly relevant to the user's questions.
    - Your primary source of truth is the meeting transcript. Try to generate responses primarily from the transcript, and then the summary or other information (unless the user asks for something specific).

    # Formatting Guidelines

    - Your response would be highly likely to be paragraphs with combined information about your thought and whatever note (in markdown format) you generated.
    - Your response would mostly be either of the two formats:
    - Suggestion of a new version of the meeting note (in markdown block format, inside ``` blocks) based on user's request. However, be careful not to create an empty markdown block.
    - Information (when it's not rewriting the note, it shouldn't be inside `blocks. Only re-written version of the note should be inside` blocks.) Try your best to put markdown notes inside ``` blocks.
    "#);

    tpl_snapshot_with_assert!(
        test_context_block_wrapped,
        ContextBlock {
            contexts: vec![SessionContext {
                title: Some("Q1 Planning".to_string()),
                date: Some("2025-03-01".to_string()),
                raw_content: None,
                enhanced_content: Some("Summary of Q1 goals.".to_string()),
                transcript: None,
                participants: vec![],
                event: None,
            }],
        },
        |v| v.starts_with("<context>") && v.trim_end().ends_with("</context>"),
        @r#"
    <context>

    Title: Q1 Planning

    Date: 2025-03-01

    Enhanced Meeting Summary:
    Summary of Q1 goals.
    </context>
    "#);
}
