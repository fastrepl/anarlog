//! `enhance-transform.ts` + `enhance-workflow.ts`'s prompt assembly and the
//! `title` task's prompts, over the shared `anlg-template-app` templates.

use anlg_template_app::{
    EnhanceSystem, EnhanceTemplate, EnhanceUser, Event, Participant, Segment, Session, Template,
    TemplateSection, TitleSystem, TitleUser, Transcript,
};

use super::summary_length::{
    SummaryLengthMode, format_summary_length_guidance, format_summary_length_mode_guidance,
    summary_length_policy,
};
use super::{Snapshot, SnapshotTranscript};

/// The settings the transform reads (`SettingValues`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptSettings {
    pub ai_language: Option<String>,
    pub auto_summary_prompt: String,
    pub summary_length: Option<String>,
    /// `personalization_dictionary_terms` (a JSON array).
    pub dictionary_terms_json: String,
}

/// A stored template (`getTemplateById`), reduced to what the prompt needs.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRecord {
    pub title: String,
    pub description: Option<String>,
    pub sections: Vec<TemplateSection>,
}

/// `TaskArgsMapTransformed["enhance"]` without the image context.
#[derive(Debug, Clone, PartialEq)]
pub struct EnhanceArgs {
    pub language: Option<String>,
    pub format_override: String,
    pub session: Session,
    pub participants: Vec<Participant>,
    pub template: Option<EnhanceTemplate>,
    pub pre_meeting_memo: String,
    pub post_meeting_memo: String,
    pub transcripts: Vec<Transcript>,
    pub summary_length: SummaryLengthMode,
    pub dictionary_terms: Vec<String>,
}

impl EnhanceArgs {
    pub fn has_template_sections(&self) -> bool {
        self.template
            .as_ref()
            .is_some_and(|template| !template.sections.is_empty())
    }
}

/// `normalizeKeywordList`.
pub fn normalize_keyword_list<'a>(words: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut result = Vec::new();
    for word in words {
        let normalized = word.split_whitespace().collect::<Vec<_>>().join(" ");
        let key = normalized.to_lowercase();
        if normalized.chars().count() < 2 || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        result.push(normalized);
    }
    result
}

/// `parseDictionaryTermsJson`.
pub fn parse_dictionary_terms_json(value: &str) -> Vec<String> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(value)
    else {
        return Vec::new();
    };
    normalize_keyword_list(items.iter().filter_map(|item| item.as_str()))
}

/// `transformArgs` for the enhance task.
pub fn enhance_args(
    snapshot: &Snapshot,
    template_id: Option<&str>,
    template_record: Option<&TemplateRecord>,
    settings: &PromptSettings,
) -> EnhanceArgs {
    let memo_sections = if template_id.is_some_and(|id| id == snapshot.raw_template_id) {
        Some(memo_template_sections(
            snapshot,
            template_record
                .map(|record| record.sections.as_slice())
                .unwrap_or(&[]),
        ))
    } else {
        None
    };
    let mut template = template_record.map(|record| EnhanceTemplate {
        title: record.title.clone(),
        description: record.description.clone(),
        sections: record.sections.clone(),
    });
    if let Some(sections) = memo_sections.filter(|sections| !sections.is_empty()) {
        template = Some(EnhanceTemplate {
            title: template_record
                .map(|record| record.title.clone())
                .unwrap_or_else(|| "Meeting memo".to_string()),
            description: template_record.and_then(|record| record.description.clone()),
            sections,
        });
    }

    let pre_meeting_memo = snapshot
        .transcripts
        .first()
        .map(|transcript| transcript.memo.clone())
        .unwrap_or_default();
    let post_meeting_memo = if snapshot.supplemental_context.trim().is_empty() {
        snapshot.raw_markdown.clone()
    } else {
        [
            snapshot.raw_markdown.as_str(),
            snapshot.supplemental_context.as_str(),
        ]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
    };

    EnhanceArgs {
        language: settings
            .ai_language
            .clone()
            .filter(|value| !value.is_empty()),
        // `getFormatOverride`: a template disables the custom format.
        format_override: if template_id.is_some() || settings.auto_summary_prompt.trim().is_empty()
        {
            String::new()
        } else {
            settings.auto_summary_prompt.clone()
        },
        session: session_data(snapshot),
        participants: snapshot
            .participants
            .iter()
            .filter(|participant| !participant.name.is_empty())
            .map(|participant| Participant {
                name: participant.name.clone(),
                job_title: Some(participant.job_title.clone()).filter(|title| !title.is_empty()),
            })
            .collect(),
        template,
        pre_meeting_memo,
        post_meeting_memo,
        transcripts: format_transcripts(snapshot),
        summary_length: SummaryLengthMode::parse(settings.summary_length.as_deref()),
        dictionary_terms: parse_dictionary_terms_json(&settings.dictionary_terms_json),
    }
}

/// `getMemoTemplateSections`: the memo's level-2 headings become sections,
/// keeping the original descriptions by title (or by position when the
/// counts match).
fn memo_template_sections(
    snapshot: &Snapshot,
    original: &[TemplateSection],
) -> Vec<TemplateSection> {
    let document = if snapshot.raw_content_format == "markdown" {
        anlg_tiptap::md_to_tiptap_json(&snapshot.raw_content).ok()
    } else {
        serde_json::from_str::<serde_json::Value>(&snapshot.raw_content).ok()
    };
    let headings: Vec<String> = document
        .as_ref()
        .and_then(|doc| doc.get("content"))
        .and_then(|content| content.as_array())
        .into_iter()
        .flatten()
        .filter(|node| {
            node.get("type").and_then(|t| t.as_str()) == Some("heading")
                && node
                    .get("attrs")
                    .and_then(|attrs| attrs.get("level"))
                    .and_then(|level| level.as_u64())
                    == Some(2)
        })
        .filter_map(|node| {
            let title = node_text(node).trim().to_string();
            (!title.is_empty()).then_some(title)
        })
        .collect();
    let preserve_positions = headings.len() == original.len();
    headings
        .into_iter()
        .enumerate()
        .map(|(index, title)| {
            let by_title = original
                .iter()
                .find(|section| section.title.trim() == title)
                .and_then(|section| section.description.clone());
            let description = by_title.or_else(|| {
                if preserve_positions {
                    original
                        .get(index)
                        .and_then(|section| section.description.clone())
                } else {
                    None
                }
            });
            TemplateSection { title, description }
        })
        .collect()
}

fn node_text(node: &serde_json::Value) -> String {
    if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
        return text.to_string();
    }
    node.get("content")
        .and_then(|c| c.as_array())
        .map(|children| children.iter().map(node_text).collect::<String>())
        .unwrap_or_default()
}

/// `getSessionData`: the event's title / times when the event parses.
fn session_data(snapshot: &Snapshot) -> Session {
    let event = serde_json::from_str::<serde_json::Value>(&snapshot.event_json)
        .ok()
        .filter(|value| value.is_object());
    let non_empty = |value: &str| (!value.is_empty()).then(|| value.to_string());
    match event {
        Some(event) => {
            let event_title = event
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            let title = non_empty(&event_title).or_else(|| non_empty(&snapshot.title));
            Session {
                title: title.clone(),
                started_at: event
                    .get("started_at")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                ended_at: event
                    .get("ended_at")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                event: Some(Event {
                    name: title.unwrap_or_default(),
                }),
            }
        }
        None => Session {
            title: non_empty(&snapshot.title),
            started_at: None,
            ended_at: None,
            event: None,
        },
    }
}

/// `formatTranscripts`: one transcript spanning the earliest start and the
/// latest end, with the rendered segments.
fn format_transcripts(snapshot: &Snapshot) -> Vec<Transcript> {
    if snapshot.segments.is_empty() || snapshot.transcripts.is_empty() {
        return Vec::new();
    }
    let started_at = snapshot
        .transcripts
        .iter()
        .map(|transcript| transcript.started_at)
        .min();
    let ended_at = snapshot
        .transcripts
        .iter()
        .map(|transcript: &SnapshotTranscript| transcript.ended_at.unwrap_or(transcript.started_at))
        .max();
    vec![Transcript {
        segments: snapshot
            .segments
            .iter()
            .map(|segment| Segment {
                speaker: segment.speaker_label.clone(),
                text: segment.text.clone(),
            })
            .collect(),
        started_at: started_at.and_then(|value| u64::try_from(value).ok()),
        ended_at: ended_at.and_then(|value| u64::try_from(value).ok()),
    }]
}

/// `formatPreferredNamesGuidance` / `appendPreferredNamesGuidance`.
pub fn append_preferred_names(prompt: &str, terms: &[String]) -> String {
    let normalized = normalize_keyword_list(terms.iter().map(String::as_str));
    if normalized.is_empty() {
        return prompt.to_string();
    }
    let list = normalized
        .iter()
        .map(|term| format!("- {term}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{prompt}\n\n# Preferred Names\n\nUse these names and terms exactly when they appear, even if the transcript or notes spell them differently:\n{list}"
    )
}

/// `getSystemPrompt`.
pub fn enhance_system_prompt(args: &EnhanceArgs) -> Result<String, String> {
    let rendered = anlg_template_app::render(Template::EnhanceSystem(EnhanceSystem {
        language: args.language.clone(),
        format_override: args.format_override.clone(),
    }))
    .map_err(|error| error.to_string())?;
    let mode_guidance =
        format_summary_length_mode_guidance(args.summary_length, args.has_template_sections());
    Ok(append_preferred_names(
        &format!("{rendered}\n\n# Summary Mode\n\n{mode_guidance}"),
        &args.dictionary_terms,
    ))
}

/// `getUserPrompt` + `withLengthGuidance` (no image context here).
/// `withLengthGuidance(withImageContextNote(getUserPrompt(args), imageCount), …)`.
pub fn enhance_user_prompt(args: &EnhanceArgs, image_count: usize) -> Result<String, String> {
    let mut rendered = anlg_template_app::render(Template::EnhanceUser(Box::new(EnhanceUser {
        session: args.session.clone(),
        participants: args.participants.clone(),
        template: args.template.clone(),
        transcripts: args.transcripts.clone(),
        pre_meeting_memo: args.pre_meeting_memo.clone(),
        post_meeting_memo: args.post_meeting_memo.clone(),
    })))
    .map_err(|error| error.to_string())?;
    if image_count > 0 {
        rendered = format!("{rendered}\n\n{}", super::images::IMAGE_CONTEXT_NOTE);
    }
    if args.has_template_sections() {
        return Ok(rendered);
    }
    let policy = summary_length_policy(&args.transcripts, args.summary_length);
    Ok(match format_summary_length_guidance(policy.as_ref()) {
        Some(guidance) => format!("{rendered}\n\n{guidance}"),
        None => rendered,
    })
}

/// `IMPORTANT: Previous attempt failed. …` on a validation retry.
pub fn with_retry_feedback(prompt: &str, feedback: &str) -> String {
    format!("{prompt}\n\nIMPORTANT: Previous attempt failed. {feedback}")
}

/// The `title` task's prompts (`title-workflow.ts`).
pub fn title_prompts(
    language: Option<&str>,
    enhanced_note: &str,
    dictionary_terms: &[String],
) -> Result<(String, String), String> {
    let system = anlg_template_app::render(Template::TitleSystem(TitleSystem {
        language: language.map(str::to_string),
    }))
    .map_err(|error| error.to_string())?;
    let user = anlg_template_app::render(Template::TitleUser(TitleUser {
        enhanced_note: enhanced_note.to_string(),
    }))
    .map_err(|error| error.to_string())?;
    Ok((append_preferred_names(&system, dictionary_terms), user))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enhancer::{EnhancedNote, SegmentPayload, SnapshotParticipant};

    fn snapshot() -> Snapshot {
        Snapshot {
            session_id: "s1".into(),
            owner_user_id: "me".into(),
            title: "Weekly sync".into(),
            created_at: String::new(),
            event_id: String::new(),
            meeting_chat: String::new(),
            raw_note_id: None,
            event_json: r#"{"title":"Planning","started_at":"2026-09-07T10:00:00Z","ended_at":""}"#
                .into(),
            raw_template_id: "tpl".into(),
            raw_content: r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Goals"}]},{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Risks"}]}]}"#.into(),
            raw_content_format: "prosemirror_json".into(),
            raw_markdown: "## Goals\n\n## Risks".into(),
            enhanced_notes: vec![EnhancedNote {
                id: "n1".into(),
                title: "Summary".into(),
                content: String::new(),
                content_format: "prosemirror_json".into(),
                template_id: String::new(),
                position: 1,
            }],
            transcripts: vec![SnapshotTranscript {
                id: "t1".into(),
                started_at: 1_000,
                ended_at: Some(5_000),
                memo: "pre".into(),
                words: vec!["hello".into(); 40],
            }],
            participants: vec![SnapshotParticipant {
                human_id: "h".into(),
                name: "Ada".into(),
                job_title: String::new(),
            }],
            segments: vec![SegmentPayload {
                speaker_label: "Ada".into(),
                start_ms: 0,
                end_ms: 1,
                text: "hello ".repeat(40).trim().into(),
            }],
            supplemental_context: "Meeting platform: Zoom".into(),
        }
    }

    #[test]
    fn args_follow_the_transform() {
        let record = TemplateRecord {
            title: "Planning".into(),
            description: Some("d".into()),
            sections: vec![
                TemplateSection {
                    title: "Goals".into(),
                    description: Some("goal desc".into()),
                },
                TemplateSection {
                    title: "Other".into(),
                    description: Some("other desc".into()),
                },
            ],
        };
        let settings = PromptSettings {
            ai_language: Some("ko".into()),
            auto_summary_prompt: "custom".into(),
            summary_length: Some("crisp".into()),
            dictionary_terms_json: r#"["Anarlog","anarlog","x"]"#.into(),
        };
        let args = enhance_args(&snapshot(), Some("tpl"), Some(&record), &settings);
        // The memo's headings replace the sections, keeping descriptions by
        // title, then by position.
        let template = args.template.unwrap();
        assert_eq!(template.title, "Planning");
        assert_eq!(
            template.sections,
            vec![
                TemplateSection {
                    title: "Goals".into(),
                    description: Some("goal desc".into())
                },
                TemplateSection {
                    title: "Risks".into(),
                    description: Some("other desc".into())
                },
            ]
        );
        // A template disables the format override.
        assert_eq!(args.format_override, "");
        assert_eq!(args.language.as_deref(), Some("ko"));
        assert_eq!(args.summary_length, SummaryLengthMode::Crisp);
        assert_eq!(args.dictionary_terms, vec!["Anarlog".to_string()]);
        assert_eq!(args.pre_meeting_memo, "pre");
        assert_eq!(
            args.post_meeting_memo,
            "## Goals\n\n## Risks\n\nMeeting platform: Zoom"
        );
        assert_eq!(args.session.title.as_deref(), Some("Planning"));
        assert_eq!(args.session.event.as_ref().unwrap().name, "Planning");
        assert_eq!(args.participants.len(), 1);
        assert_eq!(args.participants[0].job_title, None);
        assert_eq!(args.transcripts.len(), 1);
        assert_eq!(args.transcripts[0].started_at, Some(1_000));
        assert_eq!(args.transcripts[0].ended_at, Some(5_000));
        assert_eq!(args.transcripts[0].segments[0].speaker, "Ada");

        let auto = enhance_args(&snapshot(), None, None, &settings);
        assert_eq!(auto.format_override, "custom");
        assert!(auto.template.is_none());
    }

    #[test]
    fn prompts_render_with_guidance() {
        let args = enhance_args(&snapshot(), None, None, &PromptSettings::default());
        let system = enhance_system_prompt(&args).unwrap();
        assert!(system.contains("# Summary Mode"));
        assert!(system.contains("Summary mode: detailed."));
        let user = enhance_user_prompt(&args, 0).unwrap();
        assert!(user.contains("Summary length: the transcript contains about 239 characters."));
        assert!(user.contains("Ada"));
        let with_names = append_preferred_names("p", &["Zed".to_string(), "Zed".to_string()]);
        assert!(with_names.ends_with("# Preferred Names\n\nUse these names and terms exactly when they appear, even if the transcript or notes spell them differently:\n- Zed"));
        let (title_system, title_user) = title_prompts(None, "# Note", &[]).unwrap();
        assert!(!title_system.is_empty());
        assert!(title_user.contains("# Note"));
        assert!(with_retry_feedback("p", "f").ends_with("IMPORTANT: Previous attempt failed. f"));
    }
}
