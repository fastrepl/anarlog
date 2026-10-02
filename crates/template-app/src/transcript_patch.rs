use crate::common_derives;
use anlg_askama_utils::filters;

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "transcript-patch.system.md.jinja")]
    pub struct TranscriptPatchSystem {
        pub language: Option<String>,
    }
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "transcript-patch.user.md.jinja")]
    pub struct TranscriptPatchUser {
        pub transcript_json: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_askama_utils::{tpl_assert, tpl_snapshot};

    tpl_assert!(
        test_language_as_specified,
        TranscriptPatchSystem {
            language: Some("ko".to_string()),
        },
        |v| v.contains("Korean")
    );

    tpl_snapshot!(
        test_transcript_patch_user,
        TranscriptPatchUser {
            transcript_json: "{\"words\":[{\"id\":\"w1\",\"text\":\"helo\"}]}".to_string(),
        },
        @r#"
    Apply corrections to this transcript JSON document:

    {"words":[{"id":"w1","text":"helo"}]}
    "#
    );
}
