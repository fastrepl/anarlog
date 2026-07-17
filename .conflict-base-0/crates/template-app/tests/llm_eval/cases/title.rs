use hypr_template_eval::{EvalCase, EvalMessage, Expectation, Failed, PromptFragment};
use template_app::{Template, TitleSystem, TitleUser, render};

use crate::support::render_failed;

pub fn empty_note(samples: usize) -> Result<EvalCase, Failed> {
    Ok(EvalCase {
        name: "title_empty_note".to_string(),
        messages: vec![
            EvalMessage {
                role: "system".to_string(),
                content: render(Template::TitleSystem(TitleSystem { language: None }))
                    .map_err(render_failed)?,
            },
            EvalMessage {
                role: "user".to_string(),
                content: render(Template::TitleUser(TitleUser {
                    enhanced_note: String::new(),
                }))
                .map_err(render_failed)?,
            },
        ],
        prompt_fragments: vec![
            PromptFragment {
                role: "system".to_string(),
                needle: "Only output the title as plaintext".to_string(),
            },
            PromptFragment {
                role: "system".to_string(),
                needle: "output exactly: <EMPTY>".to_string(),
            },
            PromptFragment {
                role: "user".to_string(),
                needle: "SUPER CONCISE title".to_string(),
            },
        ],
        smoke_outputs: vec!["<EMPTY>".to_string()],
        expectations: vec![
            Expectation::ExactTrimmed("<EMPTY>".to_string()),
            Expectation::SingleLine,
        ],
        required_pass_rate: 0.8,
        samples,
        max_tokens: 20,
        response_format: None,
    })
}
