use std::{error::Error as StdError, future::Future};

use hypr_template_app::{
    Template, TranscriptPatchSystem, TranscriptPatchUser, render as render_template,
};
use json_patch::{Patch, patch as apply_json_patch};
use serde::{Deserialize, Serialize};

use crate::{FinalizedWord, WordState};

#[derive(Debug, Clone, Default)]
pub struct TranscriptPostprocessor {
    language: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TranscriptPostprocessorRequest {
    pub transcript_json: String,
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone)]
pub struct TranscriptPostprocessorResult {
    pub patch: Patch,
    pub corrected_words: Vec<FinalizedWord>,
    pub raw_response: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TranscriptPostprocessorError {
    #[error(transparent)]
    Template(#[from] hypr_template_app::Error),
    #[error("transcript patch runner failed")]
    Runner(#[source] Box<dyn StdError + Send + Sync>),
    #[error("failed to parse json patch response: {0}")]
    InvalidJson(serde_json::Error),
    #[error("failed to serialize transcript document: {0}")]
    SerializeDocument(serde_json::Error),
    #[error("failed to deserialize patched transcript document: {0}")]
    DeserializeDocument(serde_json::Error),
    #[error("failed to apply json patch: {0}")]
    ApplyPatch(#[from] json_patch::PatchError),
    #[error("patched transcript changed word count from {expected} to {actual}")]
    WordCountChanged { expected: usize, actual: usize },
    #[error("patched transcript changed word id at index {index}")]
    WordIdChanged { index: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EditableTranscriptDocument {
    words: Vec<EditableWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EditableWord {
    id: String,
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PatchEnvelope {
    patch: Patch,
}

impl TranscriptPostprocessor {
    pub fn new() -> Self {
        Self { language: None }
    }

    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn build_request(
        &self,
        words: &[FinalizedWord],
    ) -> Result<TranscriptPostprocessorRequest, TranscriptPostprocessorError> {
        let document = EditableTranscriptDocument::from_words(words);
        let transcript_json = serde_json::to_string_pretty(&document)
            .map_err(TranscriptPostprocessorError::SerializeDocument)?;
        let system_prompt =
            render_template(Template::TranscriptPatchSystem(TranscriptPatchSystem {
                language: self.language.clone(),
            }))?;
        let user_prompt = render_template(Template::TranscriptPatchUser(Box::new(
            TranscriptPatchUser {
                transcript_json: transcript_json.clone(),
            },
        )))?;

        Ok(TranscriptPostprocessorRequest {
            transcript_json,
            system_prompt,
            user_prompt,
        })
    }

    pub fn apply_response(
        &self,
        words: &[FinalizedWord],
        raw_response: &str,
    ) -> Result<TranscriptPostprocessorResult, TranscriptPostprocessorError> {
        if words.is_empty() {
            return Ok(TranscriptPostprocessorResult {
                patch: empty_patch(),
                corrected_words: vec![],
                raw_response: raw_response.trim().to_string(),
            });
        }

        let envelope = parse_patch_envelope(raw_response)?;
        let corrected_words = apply_patch_to_words(words, &envelope.patch)?;

        Ok(TranscriptPostprocessorResult {
            patch: envelope.patch,
            corrected_words,
            raw_response: raw_response.trim().to_string(),
        })
    }

    pub async fn process_with<F, Fut, E>(
        &self,
        words: &[FinalizedWord],
        run: F,
    ) -> Result<TranscriptPostprocessorResult, TranscriptPostprocessorError>
    where
        F: FnOnce(TranscriptPostprocessorRequest) -> Fut,
        Fut: Future<Output = Result<String, E>>,
        E: StdError + Send + Sync + 'static,
    {
        if words.is_empty() {
            return Ok(TranscriptPostprocessorResult {
                patch: empty_patch(),
                corrected_words: vec![],
                raw_response: "{\"patch\":[]}".to_string(),
            });
        }

        let request = self.build_request(words)?;
        let raw_response = run(request)
            .await
            .map_err(|err| TranscriptPostprocessorError::Runner(Box::new(err)))?;
        self.apply_response(words, &raw_response)
    }
}

impl EditableTranscriptDocument {
    fn from_words(words: &[FinalizedWord]) -> Self {
        Self {
            words: words
                .iter()
                .map(|word| EditableWord {
                    id: word.id.clone(),
                    text: word.text.clone(),
                })
                .collect(),
        }
    }
}

fn empty_patch() -> Patch {
    serde_json::from_str("[]").expect("static json patch should deserialize")
}

fn parse_patch_envelope(content: &str) -> Result<PatchEnvelope, TranscriptPostprocessorError> {
    let normalized = normalize_json_payload(content);
    serde_json::from_str(&normalized).map_err(TranscriptPostprocessorError::InvalidJson)
}

fn normalize_json_payload(content: &str) -> String {
    let trimmed = content.trim();
    let without_fences = strip_code_fences(trimmed);
    if let (Some(start), Some(end)) = (without_fences.find('{'), without_fences.rfind('}')) {
        return without_fences[start..=end].trim().to_string();
    }
    without_fences.trim().to_string()
}

fn strip_code_fences(content: &str) -> &str {
    if !content.starts_with("```") {
        return content;
    }

    let Some(first_newline) = content.find('\n') else {
        return content;
    };
    let body = &content[first_newline + 1..];
    body.strip_suffix("```")
        .map(str::trim_end)
        .unwrap_or(content)
}

fn apply_patch_to_words(
    words: &[FinalizedWord],
    patch: &Patch,
) -> Result<Vec<FinalizedWord>, TranscriptPostprocessorError> {
    let original = EditableTranscriptDocument::from_words(words);
    let mut value =
        serde_json::to_value(&original).map_err(TranscriptPostprocessorError::SerializeDocument)?;
    apply_json_patch(&mut value, patch)?;
    let patched: EditableTranscriptDocument =
        serde_json::from_value(value).map_err(TranscriptPostprocessorError::DeserializeDocument)?;

    if patched.words.len() != words.len() {
        return Err(TranscriptPostprocessorError::WordCountChanged {
            expected: words.len(),
            actual: patched.words.len(),
        });
    }

    for (index, (original_word, patched_word)) in
        original.words.iter().zip(&patched.words).enumerate()
    {
        if original_word.id != patched_word.id {
            return Err(TranscriptPostprocessorError::WordIdChanged { index });
        }
    }

    Ok(words
        .iter()
        .zip(patched.words)
        .map(|(word, patched_word)| {
            let text = if word.text.starts_with(' ') && !patched_word.text.starts_with(' ') {
                format!(" {}", patched_word.text.trim_start())
            } else {
                patched_word.text
            };
            FinalizedWord {
                id: word.id.clone(),
                text,
                start_ms: word.start_ms,
                end_ms: word.end_ms,
                channel: word.channel,
                state: WordState::Final,
                speaker_index: word.speaker_index,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_words() -> Vec<FinalizedWord> {
        vec![
            FinalizedWord {
                id: "w1".to_string(),
                text: "helo".to_string(),
                start_ms: 0,
                end_ms: 100,
                channel: 0,
                state: WordState::Pending,
                speaker_index: None,
            },
            FinalizedWord {
                id: "w2".to_string(),
                text: "wrld".to_string(),
                start_ms: 100,
                end_ms: 200,
                channel: 0,
                state: WordState::Pending,
                speaker_index: None,
            },
        ]
    }

    #[test]
    fn applies_text_replacements_and_finalizes_words() {
        let words = sample_words();
        let patch: Patch = serde_json::from_str(
            r#"[{"op":"replace","path":"/words/0/text","value":"hello"},{"op":"replace","path":"/words/1/text","value":"world"}]"#,
        )
        .unwrap();

        let corrected = apply_patch_to_words(&words, &patch).unwrap();

        assert_eq!(corrected[0].text, "hello");
        assert_eq!(corrected[1].text, "world");
        assert!(corrected.iter().all(|word| word.state == WordState::Final));
        assert_eq!(corrected[0].start_ms, 0);
        assert_eq!(corrected[1].end_ms, 200);
    }

    #[test]
    fn rejects_word_count_changes() {
        let words = sample_words();
        let patch: Patch = serde_json::from_str(r#"[{"op":"remove","path":"/words/1"}]"#).unwrap();

        let err = apply_patch_to_words(&words, &patch).unwrap_err();

        assert!(matches!(
            err,
            TranscriptPostprocessorError::WordCountChanged {
                expected: 2,
                actual: 1
            }
        ));
    }

    #[test]
    fn rejects_word_id_changes() {
        let words = sample_words();
        let patch: Patch =
            serde_json::from_str(r#"[{"op":"replace","path":"/words/0/id","value":"different"}]"#)
                .unwrap();

        let err = apply_patch_to_words(&words, &patch).unwrap_err();

        assert!(matches!(
            err,
            TranscriptPostprocessorError::WordIdChanged { index: 0 }
        ));
    }

    #[test]
    fn strips_code_fences_before_parsing() {
        let envelope = parse_patch_envelope(
            "```json\n{\"patch\":[{\"op\":\"replace\",\"path\":\"/words/0/text\",\"value\":\"hello\"}]}\n```",
        )
        .unwrap();

        let corrected = apply_patch_to_words(&sample_words(), &envelope.patch).unwrap();
        assert_eq!(corrected[0].text, "hello");
    }

    #[test]
    fn preserves_leading_space_after_correction() {
        let words = vec![
            FinalizedWord {
                id: "w1".to_string(),
                text: " hello".to_string(),
                start_ms: 0,
                end_ms: 100,
                channel: 0,
                state: WordState::Pending,
                speaker_index: None,
            },
            FinalizedWord {
                id: "w2".to_string(),
                text: " wrld".to_string(),
                start_ms: 100,
                end_ms: 200,
                channel: 0,
                state: WordState::Pending,
                speaker_index: None,
            },
        ];
        let patch: Patch =
            serde_json::from_str(r#"[{"op":"replace","path":"/words/1/text","value":"world"}]"#)
                .unwrap();

        let corrected = apply_patch_to_words(&words, &patch).unwrap();

        assert_eq!(corrected[0].text, " hello");
        assert_eq!(corrected[1].text, " world");
    }

    #[test]
    fn builds_transport_agnostic_request() {
        let request = TranscriptPostprocessor::new()
            .with_language("ko")
            .build_request(&sample_words())
            .unwrap();

        assert!(request.system_prompt.contains("Korean"));
        assert!(request.user_prompt.contains("\"words\""));
        assert!(request.transcript_json.contains("\"helo\""));
    }
}
