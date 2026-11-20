//! Cleanup of a finished dictation and editing a selection by voice, through the one model the
//! person selected. The core decides whether a model call happens and what it says, and what of
//! the reply may be typed; the caller only makes the call. There is never a second route: a
//! failure types the raw words (cleanup) or nothing (an edit).

mod accept;
mod prompt;

use serde::{Deserialize, Serialize};

use crate::context::{ContextSnapshot, MAX_BEFORE_LENGTH};
use crate::replace::{Replacement, apply_replacements};
use crate::terms::{MAX_TERMS, rank_sources};
use crate::text::{char_len, tail_chars};

/// Cleanup is a nicety: past this, the raw transcript is typed instead of waiting.
pub const CLEANUP_DEADLINE_MS: u32 = 6_000;
/// Editing a selection is the whole point, so it waits longer and never types the raw words.
pub const EDIT_DEADLINE_MS: u32 = 17_000;
pub const MAX_TEXT_LENGTH: usize = 20_000;
pub const MAX_SELECTION_LENGTH: usize = 20_000;
pub const MAX_INSTRUCTIONS_LENGTH: usize = 4_100;

/// The person's explicit choices, read at the start of the operation. Both default off.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TextPolicy {
    /// Clean up plain dictation with the selected model.
    pub cleanup: bool,
    /// Treat a dictation over selected text as an instruction to edit it.
    pub voice_edit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RefineRequest {
    pub operation_id: String,
    pub now_ms: u64,
    /// The dictation's one final transcript.
    pub text: String,
    /// The selected text, when the dictation edits it.
    pub selection: Option<String>,
    /// `None` when the person's choices couldn't be read: nothing is sent anywhere.
    pub policy: Option<TextPolicy>,
    /// Whether the selected model connection is usable now.
    pub model_ready: bool,
    pub terms: Vec<String>,
    pub replacements: Vec<Replacement>,
    pub instructions: Option<String>,
    /// The text before the caret in an Anarlog field (chat).
    pub before: Option<String>,
    /// What the native side read for this operation, after its privacy decision.
    pub context: Option<ContextSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RefineReason {
    CleanupOff,
    VoiceEditOff,
    NoModel,
    PolicyUnreadable,
    TooLong,
    TimedOut,
    Failed,
    EmptyReply,
    Malformed,
    /// The reply was much longer than what was said: it answered or added to it.
    Answered,
    /// The reply left out most of what was said.
    Dropped,
    /// The reply repeated the text already before the caret.
    RepeatsBefore,
    /// The reply looped over a phrase.
    Repetition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RefineOutcome {
    /// Type `text`, the cleaned dictation with replacements applied.
    Cleaned { text: String },
    /// Type `text`, the raw transcript with replacements applied.
    Raw { text: String, reason: RefineReason },
    /// Replace the selection with `text`, which keeps the selection's edge whitespace.
    Edited { text: String },
    /// Type nothing; the selection stays as it was.
    EditFailed { reason: RefineReason },
    /// The operation was cancelled; type nothing.
    Cancelled,
    /// Nothing was said.
    Empty,
}

/// The one model call to make, and the deadline after which it no longer counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelCall {
    pub system: String,
    pub prompt: String,
    pub deadline_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RefineKind {
    Cleanup,
    Edit,
}

/// What the reply is checked against; passed back unchanged with the reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PendingRefinement {
    pub kind: RefineKind,
    pub raw: String,
    pub selection: Option<String>,
    pub before: Option<String>,
    pub instructed: bool,
    pub replacements: Vec<Replacement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RefinePlan {
    /// No model call: the outcome is already known.
    Done { outcome: RefineOutcome },
    /// Make `call`, then resolve its reply with `pending`.
    Ask {
        call: ModelCall,
        pending: PendingRefinement,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ModelReply {
    Text {
        text: String,
    },
    /// The deadline passed first.
    TimedOut,
    /// The operation was cancelled.
    Cancelled,
    /// The model call failed; why is not carried, so no provider text is kept.
    Failed,
}

fn bounded(text: Option<&str>, limit: usize) -> Option<String> {
    text.map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| tail_chars(text, limit).to_owned())
}

/// Whether and how to refine this dictation. Decided from the request alone: the policy,
/// whether the selected model is usable, and the bounds. Context belonging to another
/// operation, or too old, is left out.
pub fn prepare_refinement(request: RefineRequest) -> RefinePlan {
    let done = |outcome| RefinePlan::Done { outcome };
    let raw = request.text.trim();
    if raw.is_empty() {
        return done(RefineOutcome::Empty);
    }
    let selection = request.selection.as_deref().filter(|s| !s.is_empty());
    let fallback = |reason| RefineOutcome::Raw {
        text: apply_replacements(raw, &request.replacements),
        reason,
    };
    let edit_failed = |reason| RefineOutcome::EditFailed { reason };
    let Some(policy) = request.policy else {
        return done(match selection {
            Some(_) => edit_failed(RefineReason::PolicyUnreadable),
            None => fallback(RefineReason::PolicyUnreadable),
        });
    };
    let context = request
        .context
        .clone()
        .and_then(|snapshot| snapshot.usable_for(&request.operation_id, request.now_ms));
    let terms = rank_sources(
        [
            request.terms.iter().map(String::as_str).collect::<Vec<_>>(),
            context
                .iter()
                .flat_map(|c| c.terms.iter().map(String::as_str))
                .collect(),
        ],
        MAX_TERMS,
    );
    let instructions = bounded(request.instructions.as_deref(), MAX_INSTRUCTIONS_LENGTH);

    if let Some(selection) = selection {
        if !policy.voice_edit {
            return done(edit_failed(RefineReason::VoiceEditOff));
        }
        if !request.model_ready {
            return done(edit_failed(RefineReason::NoModel));
        }
        if char_len(raw) > MAX_TEXT_LENGTH || char_len(selection) > MAX_SELECTION_LENGTH {
            return done(edit_failed(RefineReason::TooLong));
        }
        let prompt = prompt::edit(raw, selection, &terms, instructions.as_deref());
        return RefinePlan::Ask {
            call: ModelCall {
                system: prompt.system,
                prompt: prompt.prompt,
                deadline_ms: EDIT_DEADLINE_MS,
            },
            pending: PendingRefinement {
                kind: RefineKind::Edit,
                raw: raw.to_owned(),
                selection: Some(selection.to_owned()),
                before: None,
                instructed: instructions.is_some(),
                replacements: Vec::new(),
            },
        };
    }

    if !policy.cleanup {
        return done(fallback(RefineReason::CleanupOff));
    }
    if !request.model_ready {
        return done(fallback(RefineReason::NoModel));
    }
    if char_len(raw) > MAX_TEXT_LENGTH {
        return done(fallback(RefineReason::TooLong));
    }
    let before = bounded(request.before.as_deref(), MAX_BEFORE_LENGTH).or_else(|| {
        bounded(
            context.as_ref().and_then(|c| c.before.as_deref()),
            MAX_BEFORE_LENGTH,
        )
    });
    let screen = context.as_ref().and_then(|c| c.screen.as_deref());
    let prompt = prompt::cleanup(
        raw,
        &terms,
        screen,
        before.as_deref(),
        instructions.as_deref(),
    );
    RefinePlan::Ask {
        call: ModelCall {
            system: prompt.system,
            prompt: prompt.prompt,
            deadline_ms: CLEANUP_DEADLINE_MS,
        },
        pending: PendingRefinement {
            kind: RefineKind::Cleanup,
            raw: raw.to_owned(),
            selection: None,
            before,
            instructed: instructions.is_some(),
            replacements: request.replacements,
        },
    }
}

/// What to type once the model call settled.
pub fn resolve_refinement(pending: PendingRefinement, reply: ModelReply) -> RefineOutcome {
    match pending.kind {
        RefineKind::Cleanup => {
            let raw = |reason| RefineOutcome::Raw {
                text: apply_replacements(&pending.raw, &pending.replacements),
                reason,
            };
            match reply {
                ModelReply::Cancelled => RefineOutcome::Cancelled,
                ModelReply::TimedOut => raw(RefineReason::TimedOut),
                ModelReply::Failed => raw(RefineReason::Failed),
                ModelReply::Text { text } => match accept::accept_cleanup(
                    &pending.raw,
                    &text,
                    pending.instructed,
                    pending.before.as_deref(),
                ) {
                    Ok(cleaned) => RefineOutcome::Cleaned {
                        text: apply_replacements(&cleaned, &pending.replacements),
                    },
                    Err(reason) => raw(reason),
                },
            }
        }
        RefineKind::Edit => {
            let failed = |reason| RefineOutcome::EditFailed { reason };
            let Some(selection) = pending.selection.as_deref() else {
                return failed(RefineReason::Malformed);
            };
            match reply {
                ModelReply::Cancelled => RefineOutcome::Cancelled,
                ModelReply::TimedOut => failed(RefineReason::TimedOut),
                ModelReply::Failed => failed(RefineReason::Failed),
                ModelReply::Text { text } => match accept::accept_edit(selection, &text) {
                    Ok(text) => RefineOutcome::Edited { text },
                    Err(reason) => failed(reason),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests;
