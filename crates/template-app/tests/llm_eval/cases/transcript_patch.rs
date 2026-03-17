use hypr_template_eval::{
    CaseResponseFormat, EvalCase, EvalMessage, Expectation, Failed, PromptFragment,
};
use template_app::{Template, TranscriptPatchSystem, TranscriptPatchUser, render};

use crate::support::render_failed;

pub fn fix_typo(samples: usize) -> Result<EvalCase, Failed> {
    Ok(EvalCase {
        name: "transcript_patch_fix_typo".to_string(),
        messages: vec![
            EvalMessage {
                role: "system".to_string(),
                content: render(Template::TranscriptPatchSystem(TranscriptPatchSystem {
                    language: None,
                }))
                .map_err(render_failed)?,
            },
            EvalMessage {
                role: "user".to_string(),
                content: render(Template::TranscriptPatchUser(Box::new(
                    TranscriptPatchUser {
                        transcript_json: r#"{"words":[{"id":"w1","text":"helo"}]}"#.to_string(),
                    },
                )))
                .map_err(render_failed)?,
            },
        ],
        prompt_fragments: vec![
            PromptFragment {
                role: "system".to_string(),
                needle: r#"{"patch":[...]}"#.to_string(),
            },
            PromptFragment {
                role: "system".to_string(),
                needle: "Only use `replace` operations.".to_string(),
            },
            PromptFragment {
                role: "user".to_string(),
                needle: r#"{"words":[{"id":"w1","text":"helo"}]}"#.to_string(),
            },
        ],
        smoke_outputs: vec![
            r#"{"patch":[{"op":"replace","path":"/words/0/text","value":"hello"}]}"#.to_string(),
        ],
        expectations: vec![Expectation::JsonPatchSingleReplace {
            path: "/words/0/text".to_string(),
            value: "hello".to_string(),
        }],
        required_pass_rate: 0.8,
        samples,
        max_tokens: 128,
        response_format: Some(CaseResponseFormat::JsonObject),
    })
}

pub fn no_change(samples: usize) -> Result<EvalCase, Failed> {
    Ok(EvalCase {
        name: "transcript_patch_no_change".to_string(),
        messages: vec![
            EvalMessage {
                role: "system".to_string(),
                content: render(Template::TranscriptPatchSystem(TranscriptPatchSystem {
                    language: None,
                }))
                .map_err(render_failed)?,
            },
            EvalMessage {
                role: "user".to_string(),
                content: render(Template::TranscriptPatchUser(Box::new(
                    TranscriptPatchUser {
                        transcript_json: r#"{"words":[{"id":"w1","text":"hello"}]}"#.to_string(),
                    },
                )))
                .map_err(render_failed)?,
            },
        ],
        prompt_fragments: vec![
            PromptFragment {
                role: "system".to_string(),
                needle: r#"If no correction is needed, return {"patch":[]}"#.to_string(),
            },
            PromptFragment {
                role: "user".to_string(),
                needle: r#"{"words":[{"id":"w1","text":"hello"}]}"#.to_string(),
            },
        ],
        smoke_outputs: vec![r#"{"patch":[]}"#.to_string()],
        expectations: vec![Expectation::JsonPatchEmpty],
        required_pass_rate: 0.8,
        samples,
        max_tokens: 128,
        response_format: Some(CaseResponseFormat::JsonObject),
    })
}
