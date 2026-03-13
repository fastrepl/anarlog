use crate::common_derives;
use crate::stream;

// https://github.com/deepgram/deepgram-rust-sdk/blob/0.7.0/src/common/batch_response.rs
// https://developers.deepgram.com/reference/speech-to-text/listen-pre-recorded

common_derives! {
    #[specta(rename = "BatchWord")]
    #[cfg_attr(feature = "openapi", schema(as = BatchWord))]
    pub struct Word {
        pub word: String,
        pub start: f64,
        pub end: f64,
        pub confidence: f64,
        pub speaker: Option<usize>,
        pub punctuated_word: Option<String>,
    }
}

common_derives! {
    #[specta(rename = "BatchAlternatives")]
    #[cfg_attr(feature = "openapi", schema(as = BatchAlternatives))]
    pub struct Alternatives {
        pub transcript: String,
        pub confidence: f64,
        #[serde(default)]
        pub words: Vec<Word>,
    }
}

common_derives! {
    #[specta(rename = "BatchChannel")]
    #[cfg_attr(feature = "openapi", schema(as = BatchChannel))]
    pub struct Channel {
        pub alternatives: Vec<Alternatives>,
    }
}

common_derives! {
    #[specta(rename = "BatchResults")]
    #[cfg_attr(feature = "openapi", schema(as = BatchResults))]
    pub struct Results {
        pub channels: Vec<Channel>,
    }
}

common_derives! {
    #[specta(rename = "BatchResponse")]
    #[cfg_attr(feature = "openapi", schema(as = BatchResponse))]
    pub struct Response {
        #[cfg_attr(feature = "openapi", schema(value_type = Object))]
        pub metadata: serde_json::Value,
        pub results: Results,
    }
}

impl From<stream::Word> for Word {
    fn from(word: stream::Word) -> Self {
        Self {
            word: word.word,
            start: word.start,
            end: word.end,
            confidence: word.confidence,
            speaker: word
                .speaker
                .and_then(|speaker| (speaker >= 0).then_some(speaker as usize)),
            punctuated_word: word.punctuated_word,
        }
    }
}

impl From<stream::Alternatives> for Alternatives {
    fn from(alternatives: stream::Alternatives) -> Self {
        let transcript = alternatives.transcript.trim().to_string();
        let words = alternatives
            .words
            .into_iter()
            .map(Word::from)
            .collect::<Vec<_>>();

        Self {
            transcript,
            confidence: alternatives.confidence,
            words,
        }
    }
}

impl From<stream::Channel> for Channel {
    fn from(channel: stream::Channel) -> Self {
        let alternatives = channel
            .alternatives
            .into_iter()
            .map(Alternatives::from)
            .collect::<Vec<_>>();

        Self { alternatives }
    }
}

impl From<stream::Metadata> for serde_json::Value {
    fn from(metadata: stream::Metadata) -> Self {
        serde_json::to_value(metadata).unwrap_or_else(|_| serde_json::json!({}))
    }
}
