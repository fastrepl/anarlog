use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use specta::Type;

const MIN_SUMMARY_CHARACTERS: usize = 320;
const SECTION_GUIDANCE_CHARACTER_STEP: usize = 2_000;
const TEMPLATE_SECTION_MIN_CHARACTERS: usize = 150;
const MAX_GUIDANCE_SECTIONS: usize = 8;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum SummaryLengthMode {
    Crisp,
    Balanced,
    Detailed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct SummaryLengthGuidance {
    pub max_characters: u32,
    pub min_sections: u32,
    pub max_sections: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct SummaryLengthPolicy {
    pub mode: SummaryLengthMode,
    pub transcript_characters: u32,
    pub guidance: Option<SummaryLengthGuidance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct SummaryLengthPolicyRequest {
    pub transcript_texts: Vec<String>,
    pub mode: SummaryLengthMode,
    pub template_section_count: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct PrepareGeneratedSummaryRequest {
    pub text: String,
    pub tag_sources: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct PreparedGeneratedSummary {
    pub text: String,
    pub tag_names: Vec<String>,
    pub text_with_tags: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct ComposeGeneratedSummaryRequest {
    pub text: String,
    pub title: Option<String>,
    pub tag_names: Vec<String>,
}

pub fn count_normalized_characters(text: &str) -> usize {
    count_js_code_points(trim_js(&collapse_js_whitespace(text)))
}

pub fn summary_length_policy(
    transcript_characters: usize,
    mode: SummaryLengthMode,
    template_section_count: usize,
) -> Option<SummaryLengthPolicy> {
    if transcript_characters == 0 {
        return None;
    }

    let ratio = match mode {
        SummaryLengthMode::Crisp => 0.25_f64,
        SummaryLengthMode::Balanced => 0.5_f64,
        SummaryLengthMode::Detailed => 1.0_f64,
    };
    let transcript_characters_f64 = transcript_characters as f64;
    let base_min_sections = clamp_f64(
        (transcript_characters_f64 / (SECTION_GUIDANCE_CHARACTER_STEP * 2) as f64).ceil(),
        1.0,
        5.0,
    );
    let base_max_sections = clamp_f64(
        1.0 + (transcript_characters_f64 / SECTION_GUIDANCE_CHARACTER_STEP as f64).ceil(),
        2.0,
        MAX_GUIDANCE_SECTIONS as f64,
    );
    let minimum = MIN_SUMMARY_CHARACTERS as f64;
    let guidance_max_characters = (transcript_characters_f64 * ratio)
        .round()
        .max(minimum)
        .max((template_section_count as f64) * TEMPLATE_SECTION_MIN_CHARACTERS as f64);
    let guidance = SummaryLengthGuidance {
        max_characters: to_u32(guidance_max_characters),
        min_sections: to_u32((base_min_sections * ratio).ceil()),
        max_sections: to_u32((base_max_sections * ratio).ceil().max(2.0)),
    };

    Some(SummaryLengthPolicy {
        mode,
        transcript_characters: usize_to_u32(transcript_characters),
        guidance: Some(guidance),
    })
}

pub fn summary_length_policy_for_texts(
    texts: &[String],
    mode: SummaryLengthMode,
    template_section_count: usize,
) -> Option<SummaryLengthPolicy> {
    let joined = texts
        .iter()
        .filter(|text| !text.is_empty())
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    summary_length_policy(
        count_normalized_characters(&joined),
        mode,
        template_section_count,
    )
}

pub fn extract_tag_names(sources: &[Option<&str>]) -> Vec<String> {
    let mut names = Vec::new();
    for source in sources.iter().flatten().filter(|source| !source.is_empty()) {
        for captures in hashtag_regex().captures_iter(source) {
            if let Some(name) = captures.get(2) {
                names.push(name.as_str().to_owned());
            }
        }
    }

    normalize_tag_names(&names)
}

pub fn append_tag_line_to_markdown(markdown: &str, tag_names: &[String]) -> String {
    let normalized_tag_names = normalize_tag_names(tag_names);
    if normalized_tag_names.is_empty() {
        return markdown.to_owned();
    }

    let body = trim_end_js(&strip_trailing_tag_lines(markdown)).to_owned();
    let tag_line = normalized_tag_names
        .iter()
        .map(|tag_name| format!("#{tag_name}"))
        .collect::<Vec<_>>()
        .join(" ");

    if body.is_empty() {
        tag_line
    } else {
        format!("{body}\n\n{tag_line}")
    }
}

pub fn ensure_markdown_first_line_title(markdown: &str, title: Option<&str>) -> String {
    let Some(trimmed_title) = title.map(trim_js).filter(|title| !title.is_empty()) else {
        return markdown.to_owned();
    };

    let trimmed_markdown = trim_start_js(markdown);
    let first_line = trimmed_markdown
        .split_once('\n')
        .map_or(trimmed_markdown, |(first, _)| first);
    if first_line == format!("# {trimmed_title}") {
        return markdown.to_owned();
    }

    trim_js(&format!("# {trimmed_title}\n\n{trimmed_markdown}")).to_owned()
}

pub fn prepare_generated_summary(
    request: PrepareGeneratedSummaryRequest,
) -> Option<PreparedGeneratedSummary> {
    let text = trim_js(&request.text).to_owned();
    if text.is_empty() {
        return None;
    }

    let mut sources = Vec::with_capacity(request.tag_sources.len() + 1);
    sources.push(Some(text.as_str()));
    sources.extend(
        request
            .tag_sources
            .iter()
            .map(|source| Some(source.as_str())),
    );
    let tag_names = extract_tag_names(&sources);
    let text_with_tags = append_tag_line_to_markdown(&text, &tag_names);

    Some(PreparedGeneratedSummary {
        text,
        tag_names,
        text_with_tags,
    })
}

pub fn compose_generated_summary(request: ComposeGeneratedSummaryRequest) -> String {
    let titled = ensure_markdown_first_line_title(&request.text, request.title.as_deref());
    append_tag_line_to_markdown(trim_js(&titled), &request.tag_names)
}

fn normalize_tag_names(tag_names: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for raw_tag_name in tag_names {
        let without_hash = raw_tag_name.strip_prefix('#').unwrap_or(raw_tag_name);
        let normalized = trim_js(without_hash).to_lowercase();
        if tag_name_regex().is_match(&normalized) && seen.insert(normalized.clone()) {
            result.push(normalized);
        }
    }
    result
}

fn strip_trailing_tag_lines(markdown: &str) -> String {
    let mut lines = markdown.split('\n').collect::<Vec<_>>();
    let last_index = lines.len().saturating_sub(1);
    for (index, line) in lines.iter_mut().enumerate() {
        if index < last_index {
            *line = line.strip_suffix('\r').unwrap_or(line);
        }
    }

    let mut end = lines.len();
    while end > 0 && trim_js(lines[end - 1]).is_empty() {
        end -= 1;
    }

    while end > 0 && is_tag_only_line(lines[end - 1]) {
        end -= 1;
        while end > 0 && trim_js(lines[end - 1]).is_empty() {
            end -= 1;
        }
    }

    lines[..end].join("\n")
}

fn is_tag_only_line(line: &str) -> bool {
    let trimmed = trim_js(line);
    if trimmed.is_empty() {
        return false;
    }
    let tokens = trimmed
        .split(is_js_whitespace)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    !tokens.is_empty()
        && tokens.iter().all(|token| {
            token
                .strip_prefix('#')
                .is_some_and(|name| tag_name_regex().is_match(name))
        })
}

fn hashtag_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(^|[^\p{L}\p{N}_/#])#([\p{L}_][\p{L}\p{N}_-]*)")
            .expect("hashtag regex is valid")
    })
}

fn tag_name_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^[\p{L}_][\p{L}\p{N}_-]*$").expect("tag name regex is valid"))
}

fn collapse_js_whitespace(text: &str) -> String {
    let mut collapsed = String::with_capacity(text.len());
    let mut in_whitespace = false;
    for character in text.chars() {
        if is_js_whitespace(character) {
            if !in_whitespace {
                collapsed.push(' ');
            }
            in_whitespace = true;
        } else {
            collapsed.push(character);
            in_whitespace = false;
        }
    }
    collapsed
}

fn count_js_code_points(text: &str) -> usize {
    text.chars().count()
}

fn trim_js(text: &str) -> &str {
    trim_start_js(trim_end_js(text))
}

fn trim_start_js(text: &str) -> &str {
    let start = text
        .char_indices()
        .find(|(_, character)| !is_js_whitespace(*character))
        .map_or(text.len(), |(index, _)| index);
    &text[start..]
}

fn trim_end_js(text: &str) -> &str {
    let end = text
        .char_indices()
        .rev()
        .find(|(_, character)| !is_js_whitespace(*character))
        .map_or(0, |(index, character)| index + character.len_utf8());
    &text[..end]
}

fn is_js_whitespace(character: char) -> bool {
    matches!(
        character,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

fn clamp_f64(value: f64, minimum: f64, maximum: f64) -> f64 {
    value.min(maximum).max(minimum)
}

fn to_u32(value: f64) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else if value >= u32::MAX as f64 {
        u32::MAX
    } else {
        value as u32
    }
}

fn usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_normalized_code_points_with_javascript_whitespace() {
        assert_eq!(count_normalized_characters("이번 회의는 짧음"), 9);
        assert_eq!(count_normalized_characters("😀  meeting\tnotes"), 15);
        assert_eq!(
            count_normalized_characters("\u{FEFF}A\u{0085} B\u{FEFF}"),
            4
        );
    }

    #[test]
    fn computes_the_short_transcript_policy() {
        assert_eq!(
            summary_length_policy(200, SummaryLengthMode::Detailed, 0),
            Some(SummaryLengthPolicy {
                mode: SummaryLengthMode::Detailed,
                transcript_characters: 200,
                guidance: Some(SummaryLengthGuidance {
                    max_characters: 320,
                    min_sections: 1,
                    max_sections: 2,
                }),
            })
        );
    }

    #[test]
    fn policy_rounding_and_section_guidance_match_the_typescript_policy() {
        let cases = [
            (
                636,
                SummaryLengthMode::Detailed,
                SummaryLengthGuidance {
                    max_characters: 636,
                    min_sections: 1,
                    max_sections: 2,
                },
            ),
            (
                6_000,
                SummaryLengthMode::Detailed,
                SummaryLengthGuidance {
                    max_characters: 6_000,
                    min_sections: 2,
                    max_sections: 4,
                },
            ),
            (
                10_000,
                SummaryLengthMode::Detailed,
                SummaryLengthGuidance {
                    max_characters: 10_000,
                    min_sections: 3,
                    max_sections: 6,
                },
            ),
            (
                10_000,
                SummaryLengthMode::Balanced,
                SummaryLengthGuidance {
                    max_characters: 5_000,
                    min_sections: 2,
                    max_sections: 3,
                },
            ),
            (
                10_000,
                SummaryLengthMode::Crisp,
                SummaryLengthGuidance {
                    max_characters: 2_500,
                    min_sections: 1,
                    max_sections: 2,
                },
            ),
            (
                30_000,
                SummaryLengthMode::Detailed,
                SummaryLengthGuidance {
                    max_characters: 30_000,
                    min_sections: 5,
                    max_sections: 8,
                },
            ),
        ];

        for (characters, mode, guidance) in cases {
            let result = summary_length_policy(characters, mode, 0).unwrap();
            assert_eq!(result.guidance, Some(guidance));
        }
    }

    #[test]
    fn guidance_budget_scales_by_summary_length_mode() {
        for (mode, max_characters) in [
            (SummaryLengthMode::Crisp, 7_500),
            (SummaryLengthMode::Balanced, 15_000),
            (SummaryLengthMode::Detailed, 30_000),
        ] {
            let result = summary_length_policy(30_000, mode, 0).unwrap();
            assert_eq!(result.guidance.unwrap().max_characters, max_characters);
        }
    }

    #[test]
    fn policy_uses_the_template_section_floor() {
        let policy = summary_length_policy(160, SummaryLengthMode::Crisp, 12).unwrap();
        assert_eq!(
            policy.guidance,
            Some(SummaryLengthGuidance {
                max_characters: 1_800,
                min_sections: 1,
                max_sections: 2,
            })
        );
    }

    #[test]
    fn policy_for_texts_joins_nonempty_segments_before_counting() {
        let texts = vec!["first".to_owned(), String::new(), "second".to_owned()];
        let policy =
            summary_length_policy_for_texts(&texts, SummaryLengthMode::Detailed, 0).unwrap();
        assert_eq!(policy.transcript_characters, 12);
    }

    #[test]
    fn policy_is_absent_without_transcript_characters() {
        assert!(summary_length_policy(0, SummaryLengthMode::Detailed, 0).is_none());
    }

    #[test]
    fn extracts_unique_hashtags_in_first_insertion_order() {
        let tags = extract_tag_names(&[
            Some("# Summary\n\nDiscussed #Launch and issue #123."),
            Some("Prep #prep #launch"),
            Some("Next #follow-up"),
            Some("Template #customer"),
            Some("Use #owners"),
        ]);
        assert_eq!(tags, ["launch", "prep", "follow-up", "customer", "owners"]);
    }

    #[test]
    fn append_replaces_existing_trailing_tag_lines() {
        assert_eq!(
            append_tag_line_to_markdown(
                "Body\n\n#old #tags",
                &["old".to_owned(), "tags".to_owned(), "new".to_owned()]
            ),
            "Body\n\n#old #tags #new"
        );
    }

    #[test]
    fn appending_tags_normalizes_crlf_and_unicode_whitespace() {
        assert_eq!(
            append_tag_line_to_markdown(
                "Body\r\n\r\n#OLD\r\n",
                &[
                    "\u{FEFF}OLD\u{FEFF}".to_owned(),
                    "\u{FEFF}New\u{FEFF}".to_owned()
                ]
            ),
            "Body\n\n#old #new"
        );
    }

    #[test]
    fn ensure_title_prepends_before_markdown_summary_headings() {
        assert_eq!(
            ensure_markdown_first_line_title(
                "# Summary Section\n\n- Follow up",
                Some("Meeting Title")
            ),
            "# Meeting Title\n\n# Summary Section\n\n- Follow up"
        );
    }

    #[test]
    fn ensure_title_does_not_duplicate_an_exact_heading() {
        assert_eq!(
            ensure_markdown_first_line_title("# Meeting Title", Some("Meeting Title")),
            "# Meeting Title"
        );
    }

    #[test]
    fn prepare_generated_summary_extracts_sources_and_appends_tags() {
        let prepared = prepare_generated_summary(PrepareGeneratedSummaryRequest {
            text: "# Summary\n\nDiscussed #Launch.".to_owned(),
            tag_sources: vec!["Prep #prep #Launch".to_owned()],
        })
        .unwrap();
        assert_eq!(prepared.text, "# Summary\n\nDiscussed #Launch.");
        assert_eq!(prepared.tag_names, ["launch", "prep"]);
        assert_eq!(
            prepared.text_with_tags,
            "# Summary\n\nDiscussed #Launch.\n\n#launch #prep"
        );
    }

    #[test]
    fn prepare_generated_summary_returns_none_for_empty_text() {
        assert!(
            prepare_generated_summary(PrepareGeneratedSummaryRequest {
                text: "\u{FEFF} \n".to_owned(),
                tag_sources: Vec::new(),
            })
            .is_none()
        );
    }

    #[test]
    fn compose_adds_title_and_tag_line_with_exact_markdown_output() {
        assert_eq!(
            compose_generated_summary(ComposeGeneratedSummaryRequest {
                text: "# Summary\n\nDiscussed #Launch.".to_owned(),
                title: Some("Meeting Title".to_owned()),
                tag_names: vec!["launch".to_owned(), "prep".to_owned()],
            }),
            "# Meeting Title\n\n# Summary\n\nDiscussed #Launch.\n\n#launch #prep"
        );
    }
}
