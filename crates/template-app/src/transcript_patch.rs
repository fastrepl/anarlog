use crate::common_derives;
use hypr_askama_utils::filters;

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
    use hypr_askama_utils::{tpl_assert, tpl_snapshot};

    tpl_assert!(
        test_language_as_specified,
        TranscriptPatchSystem {
            language: Some("ko".to_string()),
        },
        |v| v.contains("Korean")
    );

    tpl_snapshot!(
        test_transcript_patch_system,
        TranscriptPatchSystem { language: None },
        fixed_date = "2025-01-01",
        @r#"
    # General Instructions

    Current date: 2025-01-01

    You correct ASR transcript words in English and respond with an RFC 6902 JSON Patch.

    # Output Contract

    - Output exactly one JSON object with this shape: {"patch":[...]}.
    - `patch` must be a valid JSON Patch array.
    - If no correction is needed, return {"patch":[]}.
    - Do not wrap the JSON in markdown code fences.
    - Do not include any explanation.

    # Patch Rules

    - The input document shape is {"words":[{"id":"...","text":"..."}]}.
    - Only use `replace` operations.
    - Only modify `/words/<index>/text`.
    - Never add, remove, reorder, or move words.
    - Never change `/words/<index>/id`.
    - Preserve the original language unless the transcript clearly contains mixed-language speech.

    # Editing Guidance

    - Fix obvious ASR mistakes, punctuation, casing, spacing, and short filler artifacts only when the correction is highly likely.
    - Prefer conservative edits. If uncertain, leave the word unchanged.
    - Keep wording faithful to what was probably spoken. Do not summarize or paraphrase.
    - When English is explicitly requested, bias corrections toward that language's standard spelling.
    "#
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
