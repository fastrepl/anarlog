use crate::common_derives;
use hypr_askama_utils::filters;

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "title.system.md.jinja")]
    pub struct TitleSystem {
        pub language: Option<String>,
    }
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "title.user.md.jinja")]
    pub struct TitleUser {
        pub enhanced_note: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_askama_utils::{tpl_assert, tpl_snapshot};

    tpl_assert!(
        test_language_as_specified,
        TitleSystem {
            language: Some("ko".to_string()),
        },
        |v| v.contains("Korean")
    );

    tpl_snapshot!(
        test_title_system,
        TitleSystem { language: None },
        fixed_date = "2025-01-01",
        @r#"
    # General Instructions

    Current date: 2025-01-01

    - You are a professional assistant that generates a perfect title for a meeting note, in English language.

    # Format Requirements

    - Only output the title as plaintext, nothing else. No characters like *"'([{}]):.
    - Never ask questions or request more information.
    - If the note is empty or has no meaningful content, output exactly: <EMPTY>
    "#);

    tpl_snapshot!(
        test_title_user,
        TitleUser {
            enhanced_note: "".to_string(),
        },
        @"
    <note>

    </note>

    Now, give me SUPER CONCISE title for above note. Only about the topic of the meeting.
    "
    );
}
