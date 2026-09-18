//! `enhance-validator.ts` and `shared/validate.ts`: the early check on the
//! first characters of a summary, and the buffered retry around the stream.

use anlg_template_app::EnhanceTemplate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    Valid,
    Invalid { feedback: String },
}

pub fn normalize_for_comparison(text: &str) -> String {
    let lowered = text.to_lowercase().replace('&', "and");
    let filtered: String = lowered
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
        .collect();
    filtered.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current.push(
                (previous[j] + cost)
                    .min(previous[j + 1] + 1)
                    .min(current[j] + 1),
            );
        }
        previous = current;
    }
    previous[b.len()]
}

/// `createEnhanceValidator`: strip a preamble before the first `#`, then
/// require an h1 and, with a template, its first section heading (within a
/// 30% edit distance).
#[derive(Debug, Clone)]
pub struct EnhanceValidator {
    require_h1: bool,
    first_section: Option<String>,
}

impl EnhanceValidator {
    pub fn new(template: Option<&EnhanceTemplate>, override_template_formatting: bool) -> Self {
        Self {
            require_h1: !override_template_formatting,
            first_section: if override_template_formatting {
                None
            } else {
                template
                    .and_then(|template| template.sections.first())
                    .map(|section| section.title.clone())
            },
        }
    }

    pub fn validate(&self, text: &str) -> Validation {
        let text = match text.find('#') {
            Some(index) if index > 0 => &text[index..],
            _ => text,
        };
        if !self.require_h1 {
            return Validation::Valid;
        }
        if !text.trim().starts_with("# ") {
            return Validation::Invalid {
                feedback: "Output must start with a markdown h1 heading (# Title).".to_string(),
            };
        }
        let Some(title) = &self.first_section else {
            return Validation::Valid;
        };
        let expected_start = format!("# {title}");
        let trimmed = text.trim();
        if expected_start.starts_with(trimmed) || trimmed.starts_with(&expected_start) {
            return Validation::Valid;
        }
        let expected = normalize_for_comparison(title);
        let actual = normalize_for_comparison(trimmed.get(2..).unwrap_or(""));
        let threshold = (expected.chars().count() as f64 * 0.3).floor() as usize;
        if levenshtein(&expected, &actual) <= threshold {
            return Validation::Valid;
        }
        Validation::Invalid {
            feedback: format!(
                "Output must start with the first template section heading: \"{expected_start}\""
            ),
        }
    }
}

/// `withEarlyValidationRetry`'s buffering decision for one attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlyCheck {
    /// Keep buffering; not enough text yet.
    Buffer,
    /// Release the buffer and pass everything through from now on.
    Flush,
    /// Abort this attempt and retry with the feedback.
    Retry,
    /// Validation failed on the last attempt; give up on checking and flush.
    GiveUp,
}

pub const EARLY_MIN_CHARS: usize = 10;
pub const EARLY_MAX_CHARS: usize = 30;
pub const EARLY_MAX_RETRIES: usize = 2;

pub fn early_check(
    validator: &EnhanceValidator,
    accumulated: &str,
    attempt: usize,
) -> (EarlyCheck, Option<String>) {
    if accumulated.trim().chars().count() >= EARLY_MIN_CHARS {
        return match validator.validate(accumulated) {
            Validation::Valid => (EarlyCheck::Flush, None),
            Validation::Invalid { feedback } => {
                if attempt + 1 < EARLY_MAX_RETRIES {
                    (EarlyCheck::Retry, Some(feedback))
                } else {
                    (EarlyCheck::GiveUp, Some(feedback))
                }
            }
        };
    }
    if accumulated.chars().count() >= EARLY_MAX_CHARS {
        return (EarlyCheck::Flush, None);
    }
    (EarlyCheck::Buffer, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_template_app::TemplateSection;

    fn template(first: &str) -> EnhanceTemplate {
        EnhanceTemplate {
            title: "T".into(),
            description: None,
            sections: vec![TemplateSection {
                title: first.into(),
                description: None,
            }],
        }
    }

    #[test]
    fn requires_an_h1_and_strips_preambles() {
        let validator = EnhanceValidator::new(None, false);
        assert_eq!(validator.validate("# Summary\n\n- a"), Validation::Valid);
        assert_eq!(
            validator.validate("Sure! Here it is:\n# Summary"),
            Validation::Valid
        );
        assert!(matches!(
            validator.validate("Summary without heading"),
            Validation::Invalid { .. }
        ));
        assert_eq!(
            EnhanceValidator::new(None, true).validate("anything"),
            Validation::Valid
        );
    }

    #[test]
    fn matches_the_first_template_section_within_distance() {
        let validator = EnhanceValidator::new(Some(&template("Key Decisions & Owners")), false);
        assert_eq!(validator.validate("# Key Dec"), Validation::Valid);
        assert_eq!(
            validator.validate("# Key Decisions and Owners\n"),
            Validation::Valid
        );
        assert_eq!(
            validator.validate("# Key Decision Owners"),
            Validation::Valid
        );
        assert!(matches!(
            validator.validate("# Something Else Entirely"),
            Validation::Invalid { feedback } if feedback.contains("# Key Decisions & Owners")
        ));
    }

    #[test]
    fn early_check_buffers_then_flushes_or_retries() {
        let validator = EnhanceValidator::new(None, false);
        assert_eq!(early_check(&validator, "# Su", 0).0, EarlyCheck::Buffer);
        assert_eq!(
            early_check(&validator, "# Summary of it", 0).0,
            EarlyCheck::Flush
        );
        assert_eq!(
            early_check(&validator, "Here is your summary", 0).0,
            EarlyCheck::Retry
        );
        assert_eq!(
            early_check(&validator, "Here is your summary", 1).0,
            EarlyCheck::GiveUp
        );
        assert_eq!(
            early_check(&validator, "                                  ", 0).0,
            EarlyCheck::Flush
        );
    }

    #[test]
    fn levenshtein_counts_edits() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("same", "same"), 0);
    }
}
