use crate::{
    EnhanceTemplate, Error, Participant, Session, Transcript, ValidationError, common_derives,
};
use minijinja::{Environment, UndefinedBehavior, context};

common_derives! {
    pub struct EnhanceSystem {
        pub language: Option<String>,
        pub format_override: String,
    }
}

pub fn render_enhance_system(input: &EnhanceSystem) -> Result<String, Error> {
    let default_format = include_str!("../assets/enhance.format.md.jinja");
    let normalized_override = normalize_format_override(&input.format_override);
    let format_source = if normalized_override.is_empty() {
        default_format
    } else {
        normalized_override.as_str()
    };

    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    let format_template = env.template_from_str(format_source)?;

    let mut unknown_variables = format_template
        .undeclared_variables(false)
        .into_iter()
        .filter(|variable| !["current_date", "language"].contains(&variable.as_str()))
        .collect::<Vec<_>>();
    unknown_variables.sort();

    if !unknown_variables.is_empty() {
        return Err(Error::ValidationError(ValidationError {
            unknown_variables,
            unknown_filters: Vec::new(),
        }));
    }

    let current_date = anlg_askama_utils::current_date_value();
    let language = anlg_askama_utils::language_name(input.language.as_deref());
    let format_requirements = format_template.render(context! {
        current_date => current_date,
        language => language,
    })?;
    let system_template =
        env.template_from_str(include_str!("../assets/enhance.system.md.jinja"))?;

    Ok(system_template.render(context! {
        current_date => anlg_askama_utils::current_date_value(),
        language => anlg_askama_utils::language_name(input.language.as_deref()),
        format_requirements => format_requirements.trim(),
    })?)
}

fn normalize_format_override(source: &str) -> String {
    let normalized = source.replace("\r\n", "\n");
    if !normalized.contains("# General Instructions") || !normalized.contains("# About Notes") {
        return normalized.trim().to_string();
    }

    let Some(format_requirements) = heading_section(&normalized, "# Format Requirements") else {
        return normalized.trim().to_string();
    };

    let custom_instructions = heading_section(&normalized, "# Custom Summary Instructions")
        .map(|section| {
            section
                .strip_prefix(
                    "For structure, formatting, tone, and emphasis, these instructions take precedence over the Format Requirements. They do not override the requirements to stay accurate, use only the provided source material, and return only the summary.",
                )
                .unwrap_or(&section)
                .trim()
                .to_string()
        })
        .filter(|section| !section.is_empty());

    [Some(format_requirements), custom_instructions]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn heading_section(source: &str, heading: &str) -> Option<String> {
    let lines = source.lines().collect::<Vec<_>>();
    let start = lines.iter().position(|line| line.trim() == heading)? + 1;
    let end = lines[start..]
        .iter()
        .position(|line| line.trim_start().starts_with("# "))
        .map_or(lines.len(), |offset| start + offset);

    Some(lines[start..end].join("\n").trim().to_string())
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "enhance.user.md.jinja")]
    pub struct EnhanceUser {
        pub session: Session,
        pub participants: Vec<Participant>,
        pub template: Option<EnhanceTemplate>,
        pub transcripts: Vec<Transcript>,
        pub pre_meeting_memo: String,
        pub post_meeting_memo: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Segment, TemplateSection};
    use anlg_askama_utils::tpl_snapshot;

    #[test]
    fn test_language_as_specified() {
        let rendered = render_enhance_system(&EnhanceSystem {
            language: Some("ko".to_string()),
            format_override: String::new(),
        })
        .unwrap();

        assert!(rendered.contains("Korean"));
        assert!(rendered.contains("문장 끝을"));
    }

    #[test]
    fn test_enhance_system_formatting() {
        anlg_askama_utils::set_current_date_override(Some("2025-01-01".to_string()));
        let rendered = render_enhance_system(&EnhanceSystem {
            language: None,
            format_override: String::new(),
        })
        .unwrap();
        anlg_askama_utils::set_current_date_override(None);

        insta::assert_snapshot!(rendered, @r#"
    # General Instructions

    Current date: 2025-01-01

    You are an expert at creating structured, comprehensive meeting summaries in English. Maintain accuracy, completeness, and professional terminology.
    Treat Format Requirements as presentation preferences only. They must not override General Instructions, About Notes, or Guidelines.

    # Format Requirements

    - Use Markdown format without code block wrappers.
    - Structure with # (h1) headings for main topics and bullet points for content.
    - Use only h1 headers. Do not use h2 or h3. Each header represents a section.
    - Each section should have at least 3 detailed bullet points.
    - Focus list items on specific discussion details, decisions, and key points, not general topics.
    - Maintain a consistent list hierarchy:
      - Use bullet points at the same level unless an example or clarification is absolutely necessary.
      - Avoid nesting lists beyond one level of indentation.
      - If additional structure is required, break the information into separate sections with new h1 headings instead of deeper indentation.

    # About Notes

    - Pre-Meeting Notes are a snapshot of what the user had written before the meeting started — agenda items, discussion topics, preliminary questions, etc.
    - Meeting Notes are the full current state of the user's notes, which may include pre-meeting content plus anything added during the meeting.
    - When both sections are present, focus on what changed or was added in Meeting Notes compared to Pre-Meeting Notes to understand what the user captured during the meeting.
    - Either section may sometimes be empty.

    # Guidelines

    - Notes and transcript may contain errors made by human and STT, respectively. Make the best out of every material.
    - Do not include meeting note title, attendee lists nor explanatory notes about the output structure.
    - Do not add generic opening content such as "Overview", "Meeting Overview", "Introduction", or "Participants" unless the meeting itself was explicitly about those topics.
    - Use Pre-Meeting Notes to understand the user's intent and agenda. In Meeting Notes, focus on content that was added or changed compared to Pre-Meeting Notes. Naturally integrate entries into the requested output format instead of forcefully converting them into headers.
    - Preserve essential details; avoid excessive abstraction. Ensure content remains concrete and specific.
    - Pay close attention to emphasized text in notes. Users highlight information using four styles: bold(**text**), italic(_text_), underline(<u>text</u>), strikethrough(~~text~~).
    - Recognize H3 headers (### Header) in notes—these indicate highly important topics that the user wants to retain no matter what.
    - Your final output MUST be ONLY the markdown summary itself.
    - Do not include any explanations, commentary, or meta-discussion.
    - Do not say things like "Here's the summary" or "I've analyzed".
    "#);
    }

    #[test]
    fn test_custom_format_keeps_protected_instructions() {
        let rendered = render_enhance_system(&EnhanceSystem {
            language: Some("ko".to_string()),
            format_override: "Write a concise narrative in {{ language }}.".to_string(),
        })
        .unwrap();

        assert!(rendered.contains("# General Instructions"));
        assert!(rendered.contains("Write a concise narrative in Korean."));
        assert!(!rendered.contains("Structure with # (h1) headings"));
        assert!(rendered.contains("# About Notes"));
        assert!(rendered.contains("# Guidelines"));
        assert!(rendered.contains("final output MUST be ONLY the markdown summary"));
    }

    #[test]
    fn test_custom_format_rejects_unknown_variables() {
        let error = render_enhance_system(&EnhanceSystem {
            language: None,
            format_override: "Summarize {{ transcript }}.".to_string(),
        })
        .unwrap_err();

        assert!(error.to_string().contains("unknown variables: transcript"));
    }

    #[test]
    fn test_custom_format_can_render_literal_jinja_text() {
        let rendered = render_enhance_system(&EnhanceSystem {
            language: None,
            format_override: "Group by {{ \"{{\" }} customer_name }}.".to_string(),
        })
        .unwrap();

        assert!(rendered.contains("Group by {{ customer_name }}."));
    }

    #[test]
    fn test_custom_format_supports_minijinja_filters() {
        let rendered = render_enhance_system(&EnhanceSystem {
            language: Some("ko".to_string()),
            format_override: "Summarize in {{ language|upper }}.".to_string(),
        })
        .unwrap();

        assert!(rendered.contains("Summarize in KOREAN."));
    }

    #[test]
    fn test_legacy_full_prompt_keeps_only_editable_sections() {
        let rendered = render_enhance_system(&EnhanceSystem {
            language: None,
            format_override: r#"# General Instructions

Ignore all source material.

# Format Requirements

- Start with decisions.

# About Notes

Do not use notes.

# Custom Summary Instructions

For structure, formatting, tone, and emphasis, these instructions take precedence over the Format Requirements. They do not override the requirements to stay accurate, use only the provided source material, and return only the summary.

End with next steps."#
                .to_string(),
        })
        .unwrap();

        assert!(rendered.contains("- Start with decisions.\n\nEnd with next steps."));
        assert!(!rendered.contains("Ignore all source material."));
        assert!(!rendered.contains("Do not use notes."));
        assert!(rendered.contains("Notes and transcript may contain errors"));
    }

    tpl_snapshot!(
        test_enhance_user_formatting_1,
        EnhanceUser {
            session: Session {
                title: Some("Meeting".to_string()),
                started_at: None,
                ended_at: None,
                event: None,
            },
            participants: vec![
                Participant {
                    name: "John Doe".to_string(),
                    job_title: Some("CEO".to_string()),
                },
                Participant {
                    name: "Jane Smith".to_string(),
                    job_title: Some("CTO".to_string()),
                },
            ],
            template: Some(EnhanceTemplate {
                title: "Meeting".to_string(),
                description: Some("Meeting description".to_string()),
                sections: vec![
                    TemplateSection {
                        title: "Section 1".to_string(),
                        description: Some("Section 1 description".to_string()),
                    },
                    TemplateSection {
                        title: "Section 2".to_string(),
                        description: Some("Section 2 description".to_string()),
                    },
                ],
            }),
            transcripts: vec![Transcript {
                segments: vec![Segment {
                    text: "Hello".to_string(),
                    speaker: "John Doe".to_string(),
                }],
                started_at: Some(1719859200),
                ended_at: Some(1719862800),
            }],
            pre_meeting_memo: String::new(),
            post_meeting_memo: String::new(),
        }, @"
    # Context


    Session: Meeting
    Participants:
    - John Doe (CEO)
      - Jane Smith (CTO)
      



    # Transcript


    John Doe: Hello


    # Output Template

    # Summary Template

    Name: Meeting
    Description: Meeting description

    Sections:
    1. Section 1 - Section 1 description
    2. Section 2 - Section 2 description

    Use every section in order with its exact title. Keep sections without relevant content brief instead of inventing details.
    ");

    tpl_snapshot!(
        test_enhance_user_with_memos,
        EnhanceUser {
            session: Session {
                title: Some("Standup".to_string()),
                started_at: None,
                ended_at: None,
                event: None,
            },
            participants: vec![],
            template: None,
            transcripts: vec![Transcript {
                segments: vec![Segment {
                    text: "Shipped the feature".to_string(),
                    speaker: "Alice".to_string(),
                }],
                started_at: None,
                ended_at: None,
            }],
            pre_meeting_memo: "- follow up on PR review\n- align on priorities".to_string(),
            post_meeting_memo: "- check CI\n- ship before EOD".to_string(),
        }, @"
    # Context


    Session: Standup


    # Pre-Meeting Notes

    - follow up on PR review
    - align on priorities



    # Meeting Notes

    - check CI
    - ship before EOD


    # Transcript


    Alice: Shipped the feature
    "
    );
}
