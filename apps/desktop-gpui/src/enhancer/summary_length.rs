//! `services/enhancer/summary-length.ts`: how long a summary may be for a
//! transcript, the prompt guidance that says so, and the post-generation
//! trim that enforces it.

use anlg_template_app::Transcript;

pub const MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY: usize = 160;
pub const SHORT_TRANSCRIPT_CHARACTER_LIMIT: usize = 1_200;
pub const MIN_SUMMARY_CHARACTERS: usize = 320;
pub const MAX_SUMMARY_GUIDANCE_CHARACTERS: usize = 7_500;
const SECTION_GUIDANCE_CHARACTER_STEP: usize = 2_000;
const MAX_GUIDANCE_SECTIONS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryLengthMode {
    Crisp,
    Balanced,
    Detailed,
}

impl SummaryLengthMode {
    /// `normalizeSummaryLengthMode`: anything unknown is `detailed`.
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("crisp") => Self::Crisp,
            Some("balanced") => Self::Balanced,
            _ => Self::Detailed,
        }
    }

    fn ratio(self) -> f64 {
        match self {
            Self::Crisp => 0.75,
            Self::Balanced => 0.875,
            Self::Detailed => 1.0,
        }
    }

    fn guidance_character_limit(self) -> usize {
        match self {
            Self::Crisp => 4_500,
            Self::Balanced => 6_000,
            Self::Detailed => MAX_SUMMARY_GUIDANCE_CHARACTERS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guidance {
    pub max_characters: usize,
    pub min_sections: usize,
    pub max_sections: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryLengthPolicy {
    pub max_characters: usize,
    pub max_sections: Option<usize>,
    pub transcript_characters: usize,
    pub guidance: Option<Guidance>,
}

/// `countNormalizedCharacters`: code points after collapsing whitespace.
pub fn count_normalized_characters(text: &str) -> usize {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .count()
}

/// `countTranscriptWordCharacters` over the words' texts.
pub fn count_word_characters<'a>(words: impl Iterator<Item = &'a str>) -> usize {
    let joined = words
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    count_normalized_characters(&joined)
}

fn ceil_ratio(value: usize, ratio: f64) -> usize {
    (value as f64 * ratio).ceil() as usize
}

pub fn summary_length_policy(
    transcripts: &[Transcript],
    mode: SummaryLengthMode,
) -> Option<SummaryLengthPolicy> {
    let joined = transcripts
        .iter()
        .flat_map(|transcript| transcript.segments.iter())
        .map(|segment| segment.text.as_str())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let transcript_characters = count_normalized_characters(&joined);
    if transcript_characters == 0 {
        return None;
    }

    let ratio = mode.ratio();
    let base_min_sections = transcript_characters
        .div_ceil(SECTION_GUIDANCE_CHARACTER_STEP * 2)
        .clamp(1, 5);
    let base_max_sections = (1 + transcript_characters.div_ceil(SECTION_GUIDANCE_CHARACTER_STEP))
        .clamp(2, MAX_GUIDANCE_SECTIONS);

    Some(SummaryLengthPolicy {
        transcript_characters,
        max_characters: ((transcript_characters.max(MIN_SUMMARY_CHARACTERS) as f64 * ratio).round()
            as usize)
            .max(MIN_SUMMARY_CHARACTERS),
        max_sections: (transcript_characters < SHORT_TRANSCRIPT_CHARACTER_LIMIT).then_some(2),
        guidance: Some(Guidance {
            max_characters: ((transcript_characters as f64 * ratio).round() as usize)
                .clamp(MIN_SUMMARY_CHARACTERS, mode.guidance_character_limit()),
            min_sections: ceil_ratio(base_min_sections, ratio),
            max_sections: ceil_ratio(base_max_sections, ratio),
        }),
    })
}

pub fn format_summary_length_mode_guidance(
    mode: SummaryLengthMode,
    has_template_sections: bool,
) -> String {
    let template_guidance = if has_template_sections {
        "Preserve every requested template section and do not add sections based on this mode."
    } else {
        "Put only explicitly stated or unambiguous owners, commitments, and deadlines in a final # Next Steps section when any exist; do not turn proposals into commitments, and count Next Steps within the overall section limit."
    };
    let list_guidance = "Write all section content as unordered Markdown list items beginning with '- '; never put prose paragraphs under a heading.";

    let parts: Vec<&str> = match mode {
        SummaryLengthMode::Crisp => vec![
            "Summary mode: crisp. Make the summary fast to scan.",
            "Cover only decisions, outcomes, blockers, commitments, and the context required to understand them.",
            "Do not omit any explicit decision, blocker, owner, commitment, or deadline.",
            "Use direct one-sentence bullets with one idea per bullet and no more than four bullets per section; a section may have fewer than three bullets.",
            "Merge closely related topics into one section before omitting secondary discussion, repetition, conversational framing, minor examples, and rationale that did not affect the outcome.",
            list_guidance,
            template_guidance,
        ],
        SummaryLengthMode::Balanced => vec![
            "Summary mode: balanced. Keep the primary discussion complete while remaining concise.",
            "Do not omit any explicit decision, blocker, owner, commitment, or deadline.",
            "Include important supporting context and rationale, but omit repetition, tangents, and minor examples.",
            "Use three to five direct bullets per section with one or two sentences per bullet.",
            list_guidance,
            template_guidance,
        ],
        SummaryLengthMode::Detailed => vec![
            "Summary mode: detailed. Capture every material topic, decision, rationale, example, open question, and commitment.",
            "Use four to seven concrete bullets per section when the source supports it, with one to three sentences and enough context to stand on their own.",
            "Retain useful secondary discussion and examples, but remove repetition and conversational filler.",
            list_guidance,
            template_guidance,
        ],
    };
    parts.join(" ")
}

pub fn format_summary_length_guidance(policy: Option<&SummaryLengthPolicy>) -> Option<String> {
    let policy = policy?;
    let guidance = policy.guidance.as_ref()?;
    let sections = if guidance.min_sections == guidance.max_sections {
        format!(
            "exactly {} section{}",
            guidance.max_sections,
            if guidance.max_sections == 1 { "" } else { "s" }
        )
    } else {
        format!(
            "{} to {} sections",
            guidance.min_sections, guidance.max_sections
        )
    };
    Some(
        [
            format!(
                "Summary length: the transcript contains about {} characters.",
                policy.transcript_characters
            ),
            format!(
                "Keep the summary proportional to it: use {sections} and stay under {} characters overall.",
                guidance.max_characters
            ),
            "A short meeting must produce a short summary; never pad with filler.".to_string(),
        ]
        .join(" "),
    )
}

/// `constrainSummaryLength`: drop sections past the limit, then cut lines
/// (and the last line at a sentence or word boundary) to the budget.
pub fn constrain_summary_length(markdown: &str, policy: Option<&SummaryLengthPolicy>) -> String {
    let Some(policy) = policy else {
        return markdown.trim().to_string();
    };

    let section_limited = limit_sections(markdown, policy.max_sections);
    if count_normalized_characters(&section_limited) <= policy.max_characters {
        return section_limited;
    }

    let mut kept: Vec<String> = Vec::new();
    for line in section_limited.split('\n') {
        let mut candidate = kept.clone();
        candidate.push(line.to_string());
        if count_normalized_characters(candidate.join("\n").trim()) <= policy.max_characters {
            kept.push(line.to_string());
            continue;
        }
        if let Some(truncated) = truncate_line_to_safe_boundary(&kept, line, policy.max_characters)
        {
            kept.push(truncated);
        }
        break;
    }

    remove_trailing_empty_heading(kept)
        .join("\n")
        .trim()
        .to_string()
}

fn is_h1(line: &str) -> bool {
    line.strip_prefix('#').is_some_and(|rest| {
        rest.starts_with(char::is_whitespace)
            && rest.trim_start().starts_with(|c: char| !c.is_whitespace())
    })
}

fn limit_sections(markdown: &str, max_sections: Option<usize>) -> String {
    let Some(max_sections) = max_sections else {
        return markdown.trim().to_string();
    };
    let mut count = 0;
    let mut kept = Vec::new();
    for line in markdown.trim().split('\n') {
        if is_h1(line) {
            count += 1;
            if count > max_sections {
                break;
            }
        }
        kept.push(line);
    }
    kept.join("\n").trim().to_string()
}

fn truncate_line_to_safe_boundary(
    kept: &[String],
    line: &str,
    max_characters: usize,
) -> Option<String> {
    let characters: Vec<char> = line.chars().collect();
    let fits = |count: usize| {
        let mut candidate: Vec<String> = kept.to_vec();
        candidate.push(characters[..count].iter().collect());
        count_normalized_characters(candidate.join("\n").trim()) <= max_characters
    };
    let (mut low, mut high) = (0usize, characters.len());
    while low < high {
        let midpoint = (low + high).div_ceil(2);
        if fits(midpoint) {
            low = midpoint;
        } else {
            high = midpoint - 1;
        }
    }

    let truncated: String = characters[..low]
        .iter()
        .collect::<String>()
        .trim_end()
        .to_string();
    // The last sentence ending followed by whitespace or the end.
    let mut last_end = None;
    let chars: Vec<char> = truncated.chars().collect();
    for (index, ch) in chars.iter().enumerate() {
        if matches!(ch, '.' | '!' | '?')
            && chars.get(index + 1).is_none_or(|next| next.is_whitespace())
        {
            last_end = Some(index);
        }
    }
    let result = match last_end {
        Some(0) | None => {
            // `^(.+\S)\s+\S*$`: drop the trailing partial word.
            let trimmed = truncated.trim_end();
            match trimmed.rfind(char::is_whitespace) {
                Some(space) if !trimmed[..space].trim_end().is_empty() => {
                    trimmed[..space].trim_end().to_string()
                }
                _ => truncated,
            }
        }
        Some(index) => chars[..=index].iter().collect(),
    };
    (!result.is_empty()).then_some(result)
}

fn remove_trailing_empty_heading(lines: Vec<String>) -> Vec<String> {
    let mut last_content = lines.len();
    while last_content > 0 && lines[last_content - 1].trim().is_empty() {
        last_content -= 1;
    }
    if last_content > 0 {
        let candidate = &lines[last_content - 1];
        let hashes = candidate.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes)
            && candidate[hashes..].starts_with(char::is_whitespace)
            && !candidate[hashes..].trim().is_empty()
        {
            return lines[..last_content - 1].to_vec();
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_template_app::Segment;

    fn transcript(text: &str) -> Vec<Transcript> {
        vec![Transcript {
            segments: vec![Segment {
                speaker: "John".into(),
                text: text.into(),
            }],
            started_at: None,
            ended_at: None,
        }]
    }

    #[test]
    fn counts_transcript_characters_across_languages() {
        assert_eq!(
            count_word_characters(["이번", "회의는", "짧음"].into_iter()),
            9
        );
    }

    #[test]
    fn caps_short_transcripts_at_two_sections() {
        let policy =
            summary_length_policy(&transcript(&"a".repeat(200)), SummaryLengthMode::Detailed)
                .unwrap();
        assert_eq!(
            policy,
            SummaryLengthPolicy {
                transcript_characters: 200,
                max_characters: 320,
                max_sections: Some(2),
                guidance: Some(Guidance {
                    max_characters: 320,
                    min_sections: 1,
                    max_sections: 2,
                }),
            }
        );
    }

    #[test]
    fn scales_the_guided_section_range() {
        let guidance = |characters: usize| {
            summary_length_policy(
                &transcript(&"a".repeat(characters)),
                SummaryLengthMode::Detailed,
            )
            .unwrap()
            .guidance
            .unwrap()
        };
        assert_eq!(
            guidance(636),
            Guidance {
                max_characters: 636,
                min_sections: 1,
                max_sections: 2
            }
        );
        assert_eq!(
            guidance(6_000),
            Guidance {
                max_characters: 6_000,
                min_sections: 2,
                max_sections: 4
            }
        );
        assert_eq!(
            guidance(30_000),
            Guidance {
                max_characters: 7_500,
                min_sections: 5,
                max_sections: 8
            }
        );
    }

    #[test]
    fn renders_proportional_guidance() {
        let policy =
            summary_length_policy(&transcript(&"a".repeat(636)), SummaryLengthMode::Detailed);
        let guidance = format_summary_length_guidance(policy.as_ref()).unwrap();
        assert!(guidance.contains("about 636 characters"));
        assert!(guidance.contains("1 to 2 sections"));
        assert!(guidance.contains("under 636 characters"));
        assert!(format_summary_length_guidance(None).is_none());
    }

    #[test]
    fn reduces_budgets_for_balanced_and_crisp() {
        let transcripts = transcript(&"a".repeat(8_000));
        let detailed = summary_length_policy(&transcripts, SummaryLengthMode::Detailed).unwrap();
        assert_eq!(detailed.max_characters, 8_000);
        let balanced = summary_length_policy(&transcripts, SummaryLengthMode::Balanced).unwrap();
        assert_eq!(balanced.max_characters, 7_000);
        let crisp = summary_length_policy(&transcripts, SummaryLengthMode::Crisp).unwrap();
        assert_eq!(crisp.max_characters, 6_000);
        assert_eq!(crisp.guidance.unwrap().max_characters, 4_500);
    }

    #[test]
    fn mode_guidance_matches_the_frontend_copy() {
        assert_eq!(SummaryLengthMode::parse(None), SummaryLengthMode::Detailed);
        assert_eq!(
            SummaryLengthMode::parse(Some("unsupported")),
            SummaryLengthMode::Detailed
        );
        assert_eq!(
            SummaryLengthMode::parse(Some("crisp")),
            SummaryLengthMode::Crisp
        );
        assert!(
            format_summary_length_mode_guidance(SummaryLengthMode::Detailed, false)
                .contains("Capture every material topic")
        );
        let crisp = format_summary_length_mode_guidance(SummaryLengthMode::Crisp, false);
        assert!(crisp.contains("one idea per bullet"));
        assert!(crisp.contains("# Next Steps"));
        assert!(
            format_summary_length_mode_guidance(SummaryLengthMode::Crisp, true)
                .contains("Preserve every requested template section")
        );
    }

    #[test]
    fn keeps_no_more_than_two_sections_or_the_budget() {
        let markdown = format!(
            "# First\n\n- {}\n\n# Second\n\n- {}\n\n# Third\n\n- {}",
            "a".repeat(40),
            "b".repeat(40),
            "c".repeat(100)
        );
        let result = constrain_summary_length(
            &markdown,
            Some(&SummaryLengthPolicy {
                transcript_characters: 160,
                max_characters: 160,
                max_sections: Some(2),
                guidance: None,
            }),
        );
        assert!(result.contains("# First"));
        assert!(result.contains("# Second"));
        assert!(!result.contains("# Third"));
        assert!(count_normalized_characters(&result) <= 160);
    }

    #[test]
    fn never_truncates_mid_sentence() {
        let result = constrain_summary_length(
            "# Decision\n\n- The team approved the launch. This additional explanation does not fit within the summary limit.\n\n# Follow-up",
            Some(&SummaryLengthPolicy {
                transcript_characters: 60,
                max_characters: 60,
                max_sections: None,
                guidance: None,
            }),
        );
        assert_eq!(result, "# Decision\n\n- The team approved the launch.");
        assert!(count_normalized_characters(&result) <= 60);
    }

    #[test]
    fn keeps_periodless_bullets_at_a_word_boundary() {
        let result = constrain_summary_length(
            "# Decision\n\n- alpha beta gamma delta epsilon zeta",
            Some(&SummaryLengthPolicy {
                transcript_characters: 30,
                max_characters: 30,
                max_sections: None,
                guidance: None,
            }),
        );
        assert_eq!(result, "# Decision\n\n- alpha beta");
        assert!(count_normalized_characters(&result) <= 30);
    }
}
