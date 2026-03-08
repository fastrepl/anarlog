use std::fmt;

use super::TranscribeOptions;

enum WhisperToken<'a> {
    StartOfPrev,
    Text(&'a str),
    StartOfTranscript,
    Language(&'a str),
    Transcribe,
    NoTimestamps,
}

impl fmt::Display for WhisperToken<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartOfPrev => write!(f, "<|startofprev|>"),
            Self::Text(s) => write!(f, "{s}"),
            Self::StartOfTranscript => write!(f, "<|startoftranscript|>"),
            Self::Language(l) => write!(f, "<|{l}|>"),
            Self::Transcribe => write!(f, "<|transcribe|>"),
            Self::NoTimestamps => write!(f, "<|notimestamps|>"),
        }
    }
}

pub(super) fn build_whisper_prompt(options: &TranscribeOptions) -> String {
    let mut tokens: Vec<WhisperToken<'_>> = Vec::new();

    if let Some(p) = &options.initial_prompt {
        tokens.push(WhisperToken::StartOfPrev);
        tokens.push(WhisperToken::Text(p));
    }

    tokens.push(WhisperToken::StartOfTranscript);

    if let Some(lang) = &options.language {
        tokens.push(WhisperToken::Language(lang.iso639_code()));
    }

    tokens.push(WhisperToken::Transcribe);
    tokens.push(WhisperToken::NoTimestamps);

    tokens.iter().map(|t| t.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use hypr_language::Language;

    use super::*;

    #[test]
    fn no_language_no_prompt() {
        let opts = TranscribeOptions::default();
        insta::assert_snapshot!(build_whisper_prompt(&opts), @"<|startoftranscript|><|transcribe|><|notimestamps|>");
    }

    #[test]
    fn with_language() {
        let opts = TranscribeOptions {
            language: Some(Language::from(hypr_language::ISO639::En)),
            ..Default::default()
        };
        insta::assert_snapshot!(build_whisper_prompt(&opts), @"<|startoftranscript|><|en|><|transcribe|><|notimestamps|>");
    }

    #[test]
    fn with_initial_prompt() {
        let opts = TranscribeOptions {
            initial_prompt: Some("Hello world".into()),
            ..Default::default()
        };
        insta::assert_snapshot!(build_whisper_prompt(&opts), @"<|startofprev|>Hello world<|startoftranscript|><|transcribe|><|notimestamps|>");
    }

    #[test]
    fn with_language_and_initial_prompt() {
        let opts = TranscribeOptions {
            language: Some(Language::from(hypr_language::ISO639::Ko)),
            initial_prompt: Some("안녕하세요".into()),
            ..Default::default()
        };
        insta::assert_snapshot!(build_whisper_prompt(&opts), @"<|startofprev|>안녕하세요<|startoftranscript|><|ko|><|transcribe|><|notimestamps|>");
    }
}
