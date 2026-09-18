//! `services/enhancer/eligibility.ts`: whether a transcript is worth a summary.

use super::summary_length::{MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY, count_word_characters};

pub const MIN_WORDS_FOR_ENHANCEMENT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipCode {
    NoTranscript,
    TranscriptTooShort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Eligibility {
    Eligible {
        character_count: usize,
        word_count: usize,
    },
    Ineligible {
        code: SkipCode,
        reason: String,
        character_count: usize,
        word_count: usize,
    },
}

impl Eligibility {
    pub fn is_eligible(&self) -> bool {
        matches!(self, Self::Eligible { .. })
    }

    pub fn word_count(&self) -> usize {
        match self {
            Self::Eligible { word_count, .. } | Self::Ineligible { word_count, .. } => *word_count,
        }
    }

    pub fn too_short(&self) -> bool {
        matches!(
            self,
            Self::Ineligible {
                code: SkipCode::TranscriptTooShort,
                ..
            }
        )
    }
}

/// `getEligibility` over each transcript's word texts.
pub fn eligibility(transcripts: &[Vec<String>]) -> Eligibility {
    if transcripts.is_empty() {
        return Eligibility::Ineligible {
            code: SkipCode::NoTranscript,
            reason: "No transcript recorded".to_string(),
            character_count: 0,
            word_count: 0,
        };
    }
    let word_count = transcripts.iter().map(Vec::len).sum::<usize>();
    let character_count = count_word_characters(transcripts.iter().flatten().map(String::as_str));
    if word_count < MIN_WORDS_FOR_ENHANCEMENT {
        return Eligibility::Ineligible {
            code: SkipCode::TranscriptTooShort,
            reason: format!(
                "Not enough words recorded ({word_count}/{MIN_WORDS_FOR_ENHANCEMENT} minimum)"
            ),
            character_count,
            word_count,
        };
    }
    if character_count < MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY {
        return Eligibility::Ineligible {
            code: SkipCode::TranscriptTooShort,
            reason: format!(
                "Transcript too short to summarize ({character_count}/{MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY} characters minimum)"
            ),
            character_count,
            word_count,
        };
    }
    Eligibility::Eligible {
        character_count,
        word_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eligibility_follows_word_and_character_floors() {
        assert!(matches!(
            eligibility(&[]),
            Eligibility::Ineligible {
                code: SkipCode::NoTranscript,
                ..
            }
        ));
        let short = eligibility(&[vec!["hi".into(), "there".into()]]);
        assert!(short.too_short());
        assert_eq!(short.word_count(), 2);
        let few_chars = eligibility(&[vec!["a".into(); 6]]);
        assert!(few_chars.too_short());
        let long: Vec<String> = (0..40).map(|_| "meeting".to_string()).collect();
        assert!(eligibility(&[long]).is_eligible());
    }
}
