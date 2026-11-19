//! Deterministic text rules for dictation, shared by the Tauri plugin and native consumers:
//! phrase replacements, fitting text at the caret, cleanup and voice-edit prompts with the
//! checks a model reply must pass, privacy decisions and bounds for window context, term
//! ranking with per-account decay, and reviewable dictionary suggestions from the person's own
//! edits. Pure: no I/O, clock, randomness or network. Behavior ported from Char; see
//! PROVENANCE.md.

mod caret;
pub mod context;
pub mod corrections;
mod refine;
mod replace;
pub mod terms;
mod text;
mod weights;

pub use caret::{CaretFit, fit_to_caret};
pub use corrections::{SuggestRequest, suggest_terms};
pub use refine::{
    CLEANUP_DEADLINE_MS, EDIT_DEADLINE_MS, ModelCall, ModelReply, PendingRefinement, RefineKind,
    RefineOutcome, RefinePlan, RefineReason, RefineRequest, TextPolicy, prepare_refinement,
    resolve_refinement,
};
pub use replace::{Replacement, apply_replacements};
pub use weights::{RankRequest, RecordRequest, TermEvent, rank_terms, record_terms};

use text::is_unspaced_writing;

// Closes what came before it, so it takes no space ahead of it.
const JOINS_WITHOUT_SPACE: &str = ".,!?;:)]}%'’”。、，！？：；）」』】〉》";

/// `next`, a newly transcribed phrase, appended to `committed`, the dictation's text so far:
/// with a space between them, except before closing punctuation and between Chinese or
/// Japanese, which are written without spaces.
pub fn join_transcript(committed: &str, next: &str) -> String {
    let text = next.trim();
    if committed.is_empty() {
        return text.to_owned();
    }
    if text.is_empty() {
        return committed.to_owned();
    }
    let first = text.chars().next().unwrap_or(' ');
    let last = committed.chars().last().unwrap_or(' ');
    if JOINS_WITHOUT_SPACE.contains(first)
        || last.is_whitespace()
        || (is_unspaced_writing(last) && is_unspaced_writing(first))
    {
        format!("{committed}{text}")
    } else {
        format!("{committed} {text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_phrases_with_one_space_except_before_closing_punctuation_or_within_cjk() {
        assert_eq!(join_transcript("", "  hello "), "hello");
        assert_eq!(join_transcript("hello", ""), "hello");
        assert_eq!(join_transcript("Send it", "to Mina."), "Send it to Mina.");
        assert_eq!(join_transcript("Send it", ", please"), "Send it, please");
        assert_eq!(
            join_transcript("明日の会議は", "十時からです"),
            "明日の会議は十時からです"
        );
        assert_eq!(join_transcript("好的", "。"), "好的。");
        assert_eq!(
            join_transcript("내일 회의는", "열 시에 해요"),
            "내일 회의는 열 시에 해요"
        );
        assert_eq!(join_transcript("確認します。", "OK"), "確認します。 OK");
    }
}
