use hypr_template_eval::{EvalCase, EvalMessage, Expectation, Failed, PromptFragment};
use template_app::{
    EnhanceSystem, EnhanceTemplate, EnhanceUser, Participant, Segment, Session, Template,
    TemplateSection, Transcript, render,
};

use crate::support::render_failed;

pub fn structured_summary(samples: usize) -> Result<EvalCase, Failed> {
    Ok(EvalCase {
        name: "enhance_structured_summary".to_string(),
        messages: vec![
            EvalMessage {
                role: "system".to_string(),
                content: render(Template::EnhanceSystem(EnhanceSystem { language: None }))
                    .map_err(render_failed)?,
            },
            EvalMessage {
                role: "user".to_string(),
                content: render(Template::EnhanceUser(Box::new(EnhanceUser {
                    session: Session {
                        title: Some("Daily Standup".to_string()),
                        started_at: None,
                        ended_at: None,
                        event: None,
                    },
                    participants: vec![
                        Participant {
                            name: "Alice".to_string(),
                            job_title: Some("Engineer".to_string()),
                        },
                        Participant {
                            name: "Bob".to_string(),
                            job_title: Some("PM".to_string()),
                        },
                    ],
                    template: Some(EnhanceTemplate {
                        title: "Daily Standup".to_string(),
                        description: Some("Team standup summary".to_string()),
                        sections: vec![
                            TemplateSection {
                                title: "Summary".to_string(),
                                description: Some("Key updates and decisions".to_string()),
                            },
                            TemplateSection {
                                title: "Action Items".to_string(),
                                description: Some("Concrete follow-ups".to_string()),
                            },
                        ],
                    }),
                    transcripts: vec![Transcript {
                        segments: vec![
                            Segment {
                                speaker: "Alice".to_string(),
                                text: "Shipped the feature and started rollout.".to_string(),
                            },
                            Segment {
                                speaker: "Bob".to_string(),
                                text: "Need to check CI before release and follow up on the PR review."
                                    .to_string(),
                            },
                        ],
                        started_at: None,
                        ended_at: None,
                    }],
                    pre_meeting_memo: "- align on priorities\n- review rollout risks".to_string(),
                    post_meeting_memo: "- check CI\n- ship before EOD".to_string(),
                })))
                .map_err(render_failed)?,
            },
        ],
        prompt_fragments: vec![
            PromptFragment {
                role: "system".to_string(),
                needle: "Use Markdown format without code block wrappers.".to_string(),
            },
            PromptFragment {
                role: "system".to_string(),
                needle: "Use only h1 headers.".to_string(),
            },
            PromptFragment {
                role: "user".to_string(),
                needle: "# Output Template".to_string(),
            },
            PromptFragment {
                role: "user".to_string(),
                needle: "1. Summary - Key updates and decisions".to_string(),
            },
            PromptFragment {
                role: "user".to_string(),
                needle: "2. Action Items - Concrete follow-ups".to_string(),
            },
        ],
        smoke_outputs: vec![
            r#"# Summary

- Alice shipped the feature and started rollout.
- Bob called out that CI must be checked before release.
- The team aligned on priorities and rollout risks.

# Action Items

- Check CI before release.
- Follow up on the PR review.
- Ship before EOD if CI passes.
"#
            .to_string(),
        ],
        expectations: vec![
            Expectation::NotContains("```".to_string()),
            Expectation::MarkdownAtLeastHeadings(2),
            Expectation::MarkdownAllHeadingsAreH1,
            Expectation::MarkdownHasHeadings(vec![
                "Summary".to_string(),
                "Action Items".to_string(),
            ]),
            Expectation::MarkdownHasUnorderedList,
            Expectation::MarkdownWordCountAtMost(220),
        ],
        required_pass_rate: 0.8,
        samples,
        max_tokens: 400,
        response_format: None,
    })
}
