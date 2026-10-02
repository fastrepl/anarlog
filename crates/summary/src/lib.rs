use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use specta::Type;

const SHORT_TRANSCRIPT_CHARACTER_LIMIT: usize = 1_200;
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
    pub max_characters: u32,
    pub max_sections: Option<u32>,
    pub transcript_characters: u32,
    pub guidance: Option<SummaryLengthGuidance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct SummaryLengthPolicyRequest {
    pub transcript_texts: Vec<String>,
    pub mode: SummaryLengthMode,
    pub custom_format: bool,
    pub template_section_count: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct PrepareGeneratedSummaryRequest {
    pub text: String,
    pub length_policy: Option<SummaryLengthPolicy>,
    pub tag_sources: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct PreparedGeneratedSummary {
    pub constrained_text: String,
    pub tag_names: Vec<String>,
    pub text_with_tags: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct ComposeGeneratedSummaryRequest {
    pub constrained_text: String,
    pub title: Option<String>,
    pub tag_names: Vec<String>,
    pub length_policy: Option<SummaryLengthPolicy>,
}

pub fn count_normalized_characters(text: &str) -> usize {
    count_js_code_points(trim_js(&collapse_js_whitespace(text)))
}

pub fn summary_length_policy(
    transcript_characters: usize,
    mode: SummaryLengthMode,
    custom_format: bool,
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
    let max_characters = ((transcript_characters_f64.max(minimum)).round()).max(minimum);
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
        max_characters: to_u32(max_characters),
        max_sections: if !custom_format && transcript_characters < SHORT_TRANSCRIPT_CHARACTER_LIMIT
        {
            Some(2)
        } else {
            None
        },
        transcript_characters: usize_to_u32(transcript_characters),
        guidance: Some(guidance),
    })
}

pub fn summary_length_policy_for_texts(
    texts: &[String],
    mode: SummaryLengthMode,
    custom_format: bool,
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
        custom_format,
        template_section_count,
    )
}

pub fn constrain_summary_length(markdown: &str, policy: Option<&SummaryLengthPolicy>) -> String {
    let Some(policy) = policy else {
        return trim_js(markdown).to_owned();
    };

    let section_limited = limit_sections(markdown, policy.max_sections);
    if count_normalized_characters(&section_limited) <= policy.max_characters as usize {
        return section_limited;
    }

    let mut kept_lines: Vec<String> = Vec::new();
    for line in section_limited.split('\n') {
        let candidate = join_lines(&kept_lines, line);
        if count_normalized_characters(trim_js(&candidate)) <= policy.max_characters as usize {
            kept_lines.push(line.to_owned());
            continue;
        }

        let truncated_line =
            truncate_line_to_safe_boundary(&kept_lines, line, policy.max_characters as usize);
        if !truncated_line.is_empty() {
            kept_lines.push(truncated_line);
        }
        break;
    }

    trim_js(&join_lines_slice(&remove_trailing_empty_heading(
        &kept_lines,
    )))
    .to_owned()
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
    let constrained_text = constrain_summary_length(&request.text, request.length_policy.as_ref());
    if constrained_text.is_empty() {
        return None;
    }

    let mut sources = Vec::with_capacity(request.tag_sources.len() + 1);
    sources.push(Some(constrained_text.as_str()));
    sources.extend(
        request
            .tag_sources
            .iter()
            .map(|source| Some(source.as_str())),
    );
    let tag_names = extract_tag_names(&sources);
    let text_with_tags = append_tag_line_to_markdown(&constrained_text, &tag_names);

    Some(PreparedGeneratedSummary {
        constrained_text,
        tag_names,
        text_with_tags,
    })
}

pub fn compose_generated_summary(request: ComposeGeneratedSummaryRequest) -> String {
    let titled =
        ensure_markdown_first_line_title(&request.constrained_text, request.title.as_deref());
    let tag_line = append_tag_line_to_markdown("", &request.tag_names);
    let reserved_tag_characters = if tag_line.is_empty() {
        0
    } else {
        count_normalized_characters(&tag_line).saturating_add(1)
    };
    let mut body_policy = request.length_policy;
    if let Some(policy) = body_policy.as_mut() {
        policy.max_characters = policy
            .max_characters
            .saturating_sub(usize_to_u32(reserved_tag_characters));
        policy.max_sections = None;
    }
    let body = constrain_summary_length(&titled, body_policy.as_ref());
    append_tag_line_to_markdown(&body, &request.tag_names)
}

fn limit_sections(markdown: &str, max_sections: Option<u32>) -> String {
    if max_sections.is_none_or(|max_sections| max_sections == 0) {
        return trim_js(markdown).to_owned();
    }

    let mut section_count = 0_u32;
    let mut kept_lines = Vec::new();
    for line in trim_js(markdown).split('\n') {
        if is_summary_heading(line) {
            section_count += 1;
            if section_count > max_sections.unwrap_or_default() {
                break;
            }
        }
        kept_lines.push(line);
    }

    trim_js(&join_lines_slice(&kept_lines)).to_owned()
}

fn truncate_line_to_safe_boundary(
    kept_lines: &[String],
    line: &str,
    max_characters: usize,
) -> String {
    let characters = line.chars().collect::<Vec<_>>();
    let mut low = 0_usize;
    let mut high = characters.len();

    while low < high {
        let midpoint = (low + high).div_ceil(2);
        let mut candidate = join_lines_slice(kept_lines);
        if !candidate.is_empty() {
            candidate.push('\n');
        }
        candidate.push_str(&characters[..midpoint].iter().collect::<String>());
        if count_normalized_characters(trim_js(&candidate)) <= max_characters {
            low = midpoint;
        } else {
            high = midpoint - 1;
        }
    }

    let truncated = trim_end_js(&characters[..low].iter().collect::<String>()).to_owned();
    match last_sentence_ending(&truncated) {
        Some((index, end)) if index != 0 => truncated[..end].to_owned(),
        _ => word_boundary_fallback(&truncated).unwrap_or(truncated),
    }
}

fn last_sentence_ending(text: &str) -> Option<(usize, usize)> {
    let mut ending = None;
    let mut characters = text.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if matches!(character, '.' | '!' | '?')
            && characters
                .peek()
                .is_none_or(|(_, next)| is_js_whitespace(*next))
        {
            ending = Some((index, index + character.len_utf8()));
        }
    }
    ending
}

fn word_boundary_fallback(text: &str) -> Option<String> {
    let characters = text.char_indices().collect::<Vec<_>>();
    let mut suffix_start = characters.len();
    while suffix_start > 0 && !is_js_whitespace(characters[suffix_start - 1].1) {
        suffix_start -= 1;
    }
    if suffix_start == 0 || suffix_start == characters.len() {
        return None;
    }

    let mut group_end = suffix_start;
    while group_end > 0 && is_js_whitespace(characters[group_end - 1].1) {
        group_end -= 1;
    }
    if group_end < 2
        || characters[..group_end]
            .iter()
            .any(|(_, character)| matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    {
        return None;
    }

    let (_, last_character) = characters[group_end - 1];
    let end = characters[group_end - 1].0 + last_character.len_utf8();
    Some(text[..end].to_owned())
}

fn remove_trailing_empty_heading(lines: &[String]) -> Vec<String> {
    let mut last_content_index = lines.len();
    while last_content_index > 0 && trim_js(&lines[last_content_index - 1]).is_empty() {
        last_content_index -= 1;
    }

    if last_content_index > 0 && is_markdown_heading(&lines[last_content_index - 1]) {
        lines[..last_content_index - 1].to_vec()
    } else {
        lines.to_vec()
    }
}

fn is_summary_heading(line: &str) -> bool {
    is_heading_with_hashes(line, 1, 1)
}

fn is_markdown_heading(line: &str) -> bool {
    is_heading_with_hashes(line, 1, 6)
}

fn is_heading_with_hashes(line: &str, minimum: usize, maximum: usize) -> bool {
    let hash_count = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(minimum..=maximum).contains(&hash_count) {
        return false;
    }

    let mut rest = line.chars().skip(hash_count);
    if !rest.next().is_some_and(is_js_whitespace) {
        return false;
    }
    rest.any(|character| !is_js_whitespace(character))
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

fn join_lines(kept_lines: &[String], line: &str) -> String {
    let mut candidate = join_lines_slice(kept_lines);
    if !candidate.is_empty() {
        candidate.push('\n');
    }
    candidate.push_str(line);
    trim_js(&candidate).to_owned()
}

fn join_lines_slice<T: AsRef<str>>(lines: &[T]) -> String {
    lines
        .iter()
        .map(|line| line.as_ref())
        .collect::<Vec<_>>()
        .join("\n")
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

    fn policy(
        mode: SummaryLengthMode,
        transcript_characters: u32,
        max_characters: u32,
        max_sections: Option<u32>,
    ) -> SummaryLengthPolicy {
        SummaryLengthPolicy {
            mode,
            max_characters,
            max_sections,
            transcript_characters,
            guidance: None,
        }
    }

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
            summary_length_policy(200, SummaryLengthMode::Detailed, false, 0),
            Some(SummaryLengthPolicy {
                mode: SummaryLengthMode::Detailed,
                max_characters: 320,
                max_sections: Some(2),
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
                636,
                Some(2),
                SummaryLengthGuidance {
                    max_characters: 636,
                    min_sections: 1,
                    max_sections: 2,
                },
            ),
            (
                6_000,
                SummaryLengthMode::Detailed,
                6_000,
                None,
                SummaryLengthGuidance {
                    max_characters: 6_000,
                    min_sections: 2,
                    max_sections: 4,
                },
            ),
            (
                10_000,
                SummaryLengthMode::Detailed,
                10_000,
                None,
                SummaryLengthGuidance {
                    max_characters: 10_000,
                    min_sections: 3,
                    max_sections: 6,
                },
            ),
            (
                10_000,
                SummaryLengthMode::Balanced,
                10_000,
                None,
                SummaryLengthGuidance {
                    max_characters: 5_000,
                    min_sections: 2,
                    max_sections: 3,
                },
            ),
            (
                10_000,
                SummaryLengthMode::Crisp,
                10_000,
                None,
                SummaryLengthGuidance {
                    max_characters: 2_500,
                    min_sections: 1,
                    max_sections: 2,
                },
            ),
            (
                30_000,
                SummaryLengthMode::Detailed,
                30_000,
                None,
                SummaryLengthGuidance {
                    max_characters: 30_000,
                    min_sections: 5,
                    max_sections: 8,
                },
            ),
        ];

        for (characters, mode, maximum, sections, guidance) in cases {
            let result = summary_length_policy(characters, mode, false, 0).unwrap();
            assert_eq!(result.max_characters, maximum);
            assert_eq!(result.max_sections, sections);
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
            let result = summary_length_policy(30_000, mode, false, 0).unwrap();
            assert_eq!(result.guidance.unwrap().max_characters, max_characters);
        }
    }

    #[test]
    fn policy_uses_the_template_section_floor_and_custom_format_rule() {
        let policy = summary_length_policy(160, SummaryLengthMode::Crisp, true, 12).unwrap();
        let standard_policy =
            summary_length_policy(160, SummaryLengthMode::Crisp, false, 0).unwrap();
        assert_eq!(policy.max_sections, None);
        assert_eq!(standard_policy.max_sections, Some(2));
        assert_eq!(policy.max_characters, standard_policy.max_characters);
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
            summary_length_policy_for_texts(&texts, SummaryLengthMode::Detailed, false, 0).unwrap();
        assert_eq!(policy.transcript_characters, 12);
    }

    #[test]
    fn policy_is_absent_without_transcript_characters() {
        assert!(summary_length_policy(0, SummaryLengthMode::Detailed, false, 0).is_none());
    }

    #[test]
    fn constrain_limits_short_summaries_to_two_sections() {
        let markdown = format!(
            "# First\n\n- {}\n\n# Second\n\n- {}\n\n# Third\n\n- {}",
            "a".repeat(40),
            "b".repeat(40),
            "c".repeat(100)
        );
        let result = constrain_summary_length(
            &markdown,
            Some(&policy(SummaryLengthMode::Detailed, 160, 160, Some(2))),
        );
        assert!(result.contains("# First"));
        assert!(result.contains("# Second"));
        assert!(!result.contains("# Third"));
        assert!(count_normalized_characters(&result) <= 160);
    }

    #[test]
    fn constrain_truncates_at_the_last_sentence_boundary() {
        let result = constrain_summary_length(
            "# Decision\n\n- The team approved the launch. This additional explanation does not fit within the summary limit.\n\n# Follow-up",
            Some(&policy(SummaryLengthMode::Detailed, 60, 60, None)),
        );
        assert_eq!(result, "# Decision\n\n- The team approved the launch.");
        assert!(count_normalized_characters(&result) <= 60);
    }

    #[test]
    fn constrain_keeps_periodless_bullets_at_a_word_boundary() {
        let result = constrain_summary_length(
            "# Decision\n\n- alpha beta gamma delta epsilon zeta",
            Some(&policy(SummaryLengthMode::Detailed, 30, 30, None)),
        );
        assert_eq!(result, "# Decision\n\n- alpha beta");
        assert!(count_normalized_characters(&result) <= 30);
    }

    #[test]
    fn sentence_ending_at_index_zero_uses_the_word_boundary_fallback() {
        let result = constrain_summary_length(
            ". alpha beta",
            Some(&policy(SummaryLengthMode::Detailed, 11, 11, None)),
        );
        assert_eq!(result, ". alpha");
    }

    #[test]
    fn constrain_removes_a_trailing_empty_heading() {
        let result = constrain_summary_length(
            "# Summary\n\nBody\n\n# Empty heading\n\n- More",
            Some(&policy(SummaryLengthMode::Detailed, 30, 30, None)),
        );
        assert_eq!(result, "# Summary\n\nBody");
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
            length_policy: None,
            tag_sources: vec!["Prep #prep #Launch".to_owned()],
        })
        .unwrap();
        assert_eq!(prepared.constrained_text, "# Summary\n\nDiscussed #Launch.");
        assert_eq!(prepared.tag_names, ["launch", "prep"]);
        assert_eq!(
            prepared.text_with_tags,
            "# Summary\n\nDiscussed #Launch.\n\n#launch #prep"
        );
    }

    #[test]
    fn prepare_generated_summary_returns_none_for_empty_constrained_text() {
        assert!(
            prepare_generated_summary(PrepareGeneratedSummaryRequest {
                text: "\u{FEFF} \n".to_owned(),
                length_policy: None,
                tag_sources: Vec::new(),
            })
            .is_none()
        );
    }

    #[test]
    fn compose_adds_title_and_tag_line_with_exact_markdown_output() {
        assert_eq!(
            compose_generated_summary(ComposeGeneratedSummaryRequest {
                constrained_text: "# Summary\n\nDiscussed #Launch.".to_owned(),
                title: Some("Meeting Title".to_owned()),
                tag_names: vec!["launch".to_owned(), "prep".to_owned()],
                length_policy: None,
            }),
            "# Meeting Title\n\n# Summary\n\nDiscussed #Launch.\n\n#launch #prep"
        );
    }

    #[test]
    fn compose_truncates_and_reserves_tag_line_characters_exactly() {
        assert_eq!(
            compose_generated_summary(ComposeGeneratedSummaryRequest {
                constrained_text: "# Summary\n\nAlpha beta gamma delta.".to_owned(),
                title: Some("Title".to_owned()),
                tag_names: vec!["launch".to_owned()],
                length_policy: Some(policy(SummaryLengthMode::Detailed, 35, 35, Some(2))),
            }),
            "# Title\n\n# Summary\n\nAlpha\n\n#launch"
        );
    }
}
