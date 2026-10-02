use crate::common_derives;
use anlg_askama_utils::filters;

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
    use anlg_askama_utils::{tpl_assert, tpl_snapshot};

    tpl_assert!(
        test_language_as_specified,
        TitleSystem {
            language: Some("ko".to_string()),
        },
        |v| v.contains("Korean")
    );

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
