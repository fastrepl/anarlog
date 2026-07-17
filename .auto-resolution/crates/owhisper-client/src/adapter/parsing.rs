use owhisper_interface::stream::Word;

pub fn ms_to_secs(ms: u64) -> f64 {
    ms as f64 / 1000.0
}

pub fn ms_to_secs_opt(ms: Option<u64>) -> f64 {
    ms.map(ms_to_secs).unwrap_or(0.0)
}

pub fn parse_speaker_id(value: &str) -> Option<usize> {
    let digits = value
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();

    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

pub trait HasTimeSpan {
    fn start_time(&self) -> f64;
    fn end_time(&self) -> f64;
}

impl HasTimeSpan for Word {
    fn start_time(&self) -> f64 {
        self.start
    }

    fn end_time(&self) -> f64 {
        self.end
    }
}

pub fn calculate_time_span<T: HasTimeSpan>(words: &[T]) -> (f64, f64) {
    match (words.first(), words.last()) {
        (Some(first), Some(last)) => {
            let start = first.start_time();
            let end = last.end_time();
            (start, end - start)
        }
        _ => (0.0, 0.0),
    }
}

pub struct WordBuilder {
    word: String,
    start: f64,
    end: f64,
    confidence: f64,
    speaker: Option<i32>,
    punctuated_word: Option<String>,
    language: Option<String>,
}

impl WordBuilder {
    pub fn new(word: impl Into<String>) -> Self {
        let word = word.into();
        Self {
            punctuated_word: Some(word.clone()),
            word,
            start: 0.0,
            end: 0.0,
            confidence: 1.0,
            speaker: None,
            language: None,
        }
    }

    pub fn start(mut self, start: f64) -> Self {
        self.start = start;
        self
    }

    pub fn end(mut self, end: f64) -> Self {
        self.end = end;
        self
    }

    pub fn confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn speaker(mut self, speaker: Option<i32>) -> Self {
        self.speaker = speaker;
        self
    }

    pub fn language(mut self, language: Option<String>) -> Self {
        self.language = language;
        self
    }

    pub fn build(self) -> Word {
        Word {
            word: self.word,
            start: self.start,
            end: self.end,
            confidence: self.confidence,
            speaker: self.speaker,
            punctuated_word: self.punctuated_word,
            language: self.language,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_speaker_id_numeric() {
        assert_eq!(parse_speaker_id("0"), Some(0));
        assert_eq!(parse_speaker_id("1"), Some(1));
        assert_eq!(parse_speaker_id("42"), Some(42));
    }

    #[test]
    fn test_parse_speaker_id_prefixed() {
        assert_eq!(parse_speaker_id("SPEAKER_0"), Some(0));
        assert_eq!(parse_speaker_id("SPEAKER_1"), Some(1));
        assert_eq!(parse_speaker_id("speaker_2"), Some(2));
    }

    #[test]
    fn test_parse_speaker_id_numeric_prefix_with_suffix() {
        assert_eq!(parse_speaker_id("1A"), Some(1));
        assert_eq!(parse_speaker_id("2B"), Some(2));
        assert_eq!(parse_speaker_id("12_right"), Some(12));
    }

    #[test]
    fn test_parse_speaker_id_invalid() {
        assert_eq!(parse_speaker_id(""), None);
        assert_eq!(parse_speaker_id("abc"), None);
    }

    #[test]
    fn test_ms_to_secs() {
        assert_eq!(ms_to_secs(0), 0.0);
        assert_eq!(ms_to_secs(1000), 1.0);
        assert_eq!(ms_to_secs(1500), 1.5);
    }

    #[test]
    fn test_ms_to_secs_opt() {
        assert_eq!(ms_to_secs_opt(None), 0.0);
        assert_eq!(ms_to_secs_opt(Some(1000)), 1.0);
        assert_eq!(ms_to_secs_opt(Some(2500)), 2.5);
    }

    #[test]
    fn test_calculate_time_span_empty() {
        let words: Vec<Word> = vec![];
        assert_eq!(calculate_time_span(&words), (0.0, 0.0));
    }

    #[test]
    fn test_calculate_time_span_single() {
        let words = vec![WordBuilder::new("hello").start(1.0).end(2.0).build()];
        assert_eq!(calculate_time_span(&words), (1.0, 1.0));
    }

    #[test]
    fn test_calculate_time_span_multiple() {
        let words = vec![
            WordBuilder::new("hello").start(1.0).end(2.0).build(),
            WordBuilder::new("world").start(2.5).end(3.5).build(),
        ];
        assert_eq!(calculate_time_span(&words), (1.0, 2.5));
    }

    #[test]
    fn test_word_builder() {
        let word = WordBuilder::new("test")
            .start(1.5)
            .end(2.5)
            .confidence(0.95)
            .speaker(Some(1))
            .language(Some("en".to_string()))
            .build();

        assert_eq!(word.word, "test");
        assert_eq!(word.start, 1.5);
        assert_eq!(word.end, 2.5);
        assert_eq!(word.confidence, 0.95);
        assert_eq!(word.speaker, Some(1));
        assert_eq!(word.punctuated_word, Some("test".to_string()));
        assert_eq!(word.language, Some("en".to_string()));
    }
}
