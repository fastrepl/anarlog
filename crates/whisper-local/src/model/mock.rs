use crate::Segment;
use hypr_whisper::Language;

#[derive(Default)]
pub struct LoadedWhisperBuilder {}

pub struct LoadedWhisper {}

impl LoadedWhisperBuilder {
    pub fn model_path(self, _model_path: impl Into<String>) -> Self {
        self
    }

    pub fn build(self) -> Result<LoadedWhisper, crate::Error> {
        Ok(LoadedWhisper {})
    }
}

impl LoadedWhisper {
    pub fn builder() -> LoadedWhisperBuilder {
        LoadedWhisperBuilder::default()
    }

    pub fn session(&self, languages: Vec<Language>) -> Result<Whisper, crate::Error> {
        Ok(Whisper {
            languages,
            dynamic_prompt: String::new(),
        })
    }
}

#[derive(Default)]
pub struct WhisperBuilder {
    model_path: Option<String>,
    languages: Option<Vec<Language>>,
}

pub struct Whisper {
    languages: Vec<Language>,
    dynamic_prompt: String,
}

impl WhisperBuilder {
    pub fn model_path(mut self, model_path: impl Into<String>) -> Self {
        self.model_path = Some(model_path.into());
        self
    }

    pub fn languages(mut self, languages: Vec<Language>) -> Self {
        self.languages = Some(languages);
        self
    }

    pub fn build(self) -> Result<Whisper, crate::Error> {
        let _ = self.model_path;
        LoadedWhisper::builder()
            .build()?
            .session(self.languages.unwrap_or_default())
    }
}

impl Whisper {
    pub fn builder() -> WhisperBuilder {
        WhisperBuilder::default()
    }

    pub fn transcribe(&mut self, _samples: &[f32]) -> Result<Vec<Segment>, crate::Error> {
        Ok(vec![Segment {
            text: "mock".to_string(),
            language: None,
            start: 0.0,
            end: 1.0,
            confidence: 1.0,
            meta: None,
        }])
    }
}
