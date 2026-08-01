use std::path::Path;
use std::str::FromStr;
use std::time::Instant;

use owhisper_interface::{batch, stream};

pub const LOCAL_BASE_URL: &str = "apple-speech://local";
pub const MODEL_ID: &str = "apple-speech";

const MIN_SYNTHETIC_DURATION_SECONDS: f64 = 0.05;

pub fn is_local_base_url(base_url: &str) -> bool {
    base_url.trim_end_matches('/') == LOCAL_BASE_URL
}

pub fn local_model_from_request(base_url: &str, model: &str) -> Option<AppleSpeechModel> {
    let model = model.parse().ok()?;
    is_local_base_url(base_url).then_some(model)
}

/// Apple's Speech framework exposes a single on-device transcriber whose behavior is
/// selected by locale rather than by model, so this carries one variant.
#[derive(
    Debug, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type, Eq, Hash, PartialEq,
)]
pub enum AppleSpeechModel {
    #[serde(rename = "apple-speech")]
    Default,
}

impl AppleSpeechModel {
    const ALL: &'static [Self] = &[Self::Default];

    pub const fn all() -> &'static [Self] {
        Self::ALL
    }

    pub const fn selectable() -> &'static [Self] {
        Self::ALL
    }

    pub const fn as_str(self) -> &'static str {
        MODEL_ID
    }

    pub const fn display_name(self) -> &'static str {
        "Apple Speech"
    }

    pub const fn description(self) -> &'static str {
        "Realtime transcription built into macOS 26."
    }

    /// Assets are installed and shared by macOS, so nothing is downloaded from our infra.
    pub const fn size_bytes(self) -> u64 {
        0
    }

    pub const fn supports_live(self) -> bool {
        true
    }

    pub fn is_available_on_current_platform(self) -> bool {
        cfg!(target_os = "macos") && availability().is_ok_and(|value| value.status == "available")
    }

    pub fn supports_live_on_current_platform(self) -> bool {
        self.is_available_on_current_platform()
    }

    pub const fn batch_model(self) -> Self {
        self
    }

    /// "Supported" means transcribable right now: Apple Speech has the language *and*
    /// the user added it in System Settings, which is what makes it installable.
    pub fn supports_language(self, language: &anlg_language::Language) -> bool {
        let requested = language.bcp47_code();
        let Ok(preferred) = preferred_locales() else {
            return false;
        };

        preferred
            .iter()
            .any(|locale| locale_matches(locale, &requested))
    }

    /// Whether Apple Speech can transcribe the language at all, ignoring System Settings.
    /// Distinguishes "add it in System Settings" from "pick another model".
    pub fn can_transcribe_language(self, language: &anlg_language::Language) -> bool {
        let requested = language.bcp47_code();
        let Ok(supported) = supported_locales() else {
            return false;
        };

        supported
            .iter()
            .any(|locale| locale_matches(locale, &requested))
    }

    pub fn supports_languages(self, languages: &[anlg_language::Language]) -> bool {
        languages
            .iter()
            .all(|language| self.supports_language(language))
    }
}

/// `en-US` should satisfy a request for `en`, and an exact tag should match itself.
fn locale_matches(supported: &str, requested: &str) -> bool {
    if supported.eq_ignore_ascii_case(requested) {
        return true;
    }

    let supported_primary = supported.split('-').next().unwrap_or_default();
    let requested_primary = requested.split('-').next().unwrap_or_default();

    !requested.contains('-') && supported_primary.eq_ignore_ascii_case(requested_primary)
}

impl std::fmt::Display for AppleSpeechModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AppleSpeechModel {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        if value == MODEL_ID {
            Ok(Self::Default)
        } else {
            Err(Error::UnsupportedModel(value.to_string()))
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadState {
    pub status: String,
    pub current_file: Option<String>,
    pub progress_percent: Option<u8>,
    pub local_path: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTranscript {
    pub text: String,
    pub duration_seconds: f64,
    #[serde(default)]
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptSource {
    Microphone,
    System,
}

impl TranscriptSource {
    #[cfg(target_os = "macos")]
    const fn as_str(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::System => "system",
        }
    }

    pub const fn channel_index(self) -> i32 {
        match self {
            Self::Microphone => 0,
            Self::System => 1,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePartial {
    pub source: String,
    pub text: String,
    pub is_final: bool,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub words: Vec<Word>,
}

impl LivePartial {
    pub fn source(&self) -> TranscriptSource {
        match self.source.as_str() {
            "system" => TranscriptSource::System,
            _ => TranscriptSource::Microphone,
        }
    }

    pub fn into_stream_response(self) -> stream::StreamResponse {
        let source = self.source();
        let channel_index = vec![source.channel_index(), 2];
        let start = self.start.max(0.0);
        let duration = (self.end - self.start).max(MIN_SYNTHETIC_DURATION_SECONDS);
        let text = normalize_transcript_text(&self.text);
        let is_final = self.is_final;

        let words = if self.words.is_empty() {
            stream_words_from_text(&text, start, duration)
        } else {
            self.words
                .into_iter()
                .map(|word| stream::Word {
                    word: word.text.clone(),
                    start: word.start,
                    end: word.end.max(word.start + MIN_SYNTHETIC_DURATION_SECONDS),
                    confidence: word.confidence.unwrap_or(1.0),
                    speaker: None,
                    punctuated_word: Some(word.text),
                    language: None,
                })
                .collect()
        };

        stream::StreamResponse::TranscriptResponse {
            start,
            duration,
            is_final,
            speech_final: is_final,
            from_finalize: false,
            channel: stream::Channel {
                alternatives: vec![stream::Alternatives {
                    transcript: text,
                    words,
                    confidence: 1.0,
                    languages: vec![],
                }],
            },
            metadata: metadata(),
            channel_index,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported Apple Speech model: {0}")]
    UnsupportedModel(String),
    #[error("Apple Speech is only available on macOS")]
    UnsupportedPlatform,
    #[error("Apple Speech requires macOS 26 or newer")]
    RequiresMacOs26,
    #[error(
        "Apple Speech can only transcribe languages added in System Settings; none of yours are supported."
    )]
    NoSupportedSystemLanguage,
    #[error("Apple Speech bridge failed: {0}")]
    Bridge(String),
    #[error("failed to parse Apple Speech bridge response: {0}")]
    ResponseParse(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn availability() -> Result<Availability> {
    if !cfg!(target_os = "macos") {
        return Err(Error::UnsupportedPlatform);
    }

    platform::availability()
}

fn ensure_available() -> Result<()> {
    let availability = availability()?;

    match availability.status.as_str() {
        "available" => Ok(()),
        "unsupported" => Err(Error::RequiresMacOs26),
        _ => Err(Error::Bridge(availability.reason.unwrap_or_else(|| {
            "Apple Speech is unavailable on this Mac.".to_string()
        }))),
    }
}

pub fn supported_locales() -> Result<Vec<String>> {
    if !cfg!(target_os = "macos") {
        return Err(Error::UnsupportedPlatform);
    }

    platform::supported_locales()
}

/// Locales the user has explicitly added in System Settings that Apple Speech can also
/// transcribe. Installs are constrained to this set, so adding a language in System
/// Settings is what makes it available to the app.
pub fn preferred_locales() -> Result<Vec<String>> {
    if !cfg!(target_os = "macos") {
        return Err(Error::UnsupportedPlatform);
    }

    platform::preferred_locales()
}

/// Resolves the locale a session should run in: the first requested language the user
/// has also added in System Settings. `None` means the language is unavailable until
/// they add it there.
pub fn resolve_session_locale(languages: &[anlg_language::Language]) -> Option<String> {
    let preferred = preferred_locales().ok()?;

    if languages.is_empty() {
        return preferred.first().cloned();
    }

    languages.iter().find_map(|language| {
        let requested = language.bcp47_code();
        preferred
            .iter()
            .find(|locale| locale_matches(locale, &requested))
            .cloned()
    })
}

pub fn installed_locales() -> Result<Vec<String>> {
    if !cfg!(target_os = "macos") {
        return Err(Error::UnsupportedPlatform);
    }

    platform::installed_locales()
}

pub fn model_download_state(locale: &str) -> Result<ModelDownloadState> {
    ensure_available()?;
    platform::download_state(locale)
}

pub fn start_model_download(locale: &str) -> Result<()> {
    ensure_available()?;
    platform::start_download(locale)
}

pub fn release_locale(locale: &str) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(Error::UnsupportedPlatform);
    }

    platform::release_locale(locale)
}

pub fn is_model_downloaded(locale: &str) -> Result<bool> {
    Ok(model_download_state(locale)?.status == "ready")
}

pub fn is_model_downloading(locale: &str) -> Result<bool> {
    Ok(model_download_state(locale)?.status == "downloading")
}

/// Transcribes raw 16kHz mono samples, used by the batch path after it splits a
/// recording into per-source channels.
pub fn transcribe_samples(samples: &[f32], locale: &str) -> Result<FileTranscript> {
    ensure_available()?;
    platform::transcribe_samples(samples, locale)
}

pub fn transcribe_file(path: impl AsRef<Path>, locale: &str) -> Result<FileTranscript> {
    ensure_available()?;

    let path = path.as_ref();
    let started_at = Instant::now();

    tracing::info!(
        anarlog.stt.provider.name = "apple-speech",
        anarlog.stt.language = %locale,
        "apple_speech_file_transcription_start"
    );

    let result = platform::transcribe_file(path, locale);
    let elapsed_ms = started_at.elapsed().as_millis() as u64;

    match &result {
        Ok(transcript) => tracing::info!(
            anarlog.stt.provider.name = "apple-speech",
            elapsed_ms,
            transcript.duration_seconds = transcript.duration_seconds,
            transcript.text_chars = transcript.text.chars().count(),
            "apple_speech_file_transcription_completed"
        ),
        Err(error) => tracing::error!(
            anarlog.stt.provider.name = "apple-speech",
            elapsed_ms,
            error = %error,
            "apple_speech_file_transcription_failed"
        ),
    }

    result
}

pub struct LiveTranscriptionSession {
    stopped: bool,
}

impl LiveTranscriptionSession {
    pub fn start(locale: &str) -> Result<Self> {
        ensure_available()?;
        platform::live_start(locale)?;
        Ok(Self { stopped: false })
    }

    pub fn append(
        &mut self,
        source: TranscriptSource,
        samples: &[f32],
    ) -> Result<Vec<LivePartial>> {
        platform::live_append(source, samples)
    }

    pub fn finalize(&mut self, source: TranscriptSource) -> Result<Vec<LivePartial>> {
        platform::live_finalize(source)
    }

    pub fn stop(mut self) -> Result<()> {
        self.stopped = true;
        platform::live_stop()
    }
}

impl Drop for LiveTranscriptionSession {
    fn drop(&mut self) {
        if !self.stopped {
            let _ = platform::live_stop();
        }
    }
}

pub fn batch_response_from_transcripts(channels: Vec<FileTranscript>) -> batch::Response {
    let channels = if channels.is_empty() {
        vec![FileTranscript {
            text: String::new(),
            duration_seconds: MIN_SYNTHETIC_DURATION_SECONDS,
            words: Vec::new(),
        }]
    } else {
        channels
    };

    let duration_seconds = channels
        .iter()
        .map(|channel| channel.duration_seconds)
        .fold(MIN_SYNTHETIC_DURATION_SECONDS, f64::max);
    let metadata = metadata_json(duration_seconds, channels.len() as u32);

    batch::Response {
        metadata,
        results: batch::Results {
            channels: channels
                .into_iter()
                .enumerate()
                .map(|(channel_index, channel)| {
                    let transcript = normalize_transcript_text(&channel.text);
                    let words = channel
                        .words
                        .into_iter()
                        .map(|word| batch::Word {
                            word: word.text.clone(),
                            start: word.start,
                            end: word.end.max(word.start + MIN_SYNTHETIC_DURATION_SECONDS),
                            confidence: word.confidence.unwrap_or(1.0),
                            channel: channel_index as i32,
                            speaker: None,
                            punctuated_word: Some(word.text),
                        })
                        .collect();

                    batch::Channel {
                        alternatives: vec![batch::Alternatives {
                            words,
                            transcript,
                            confidence: 1.0,
                        }],
                    }
                })
                .collect(),
        },
    }
}

fn metadata() -> stream::Metadata {
    stream::Metadata {
        model_info: stream::ModelInfo {
            name: MODEL_ID.to_string(),
            version: "1".to_string(),
            arch: "apple-speech".to_string(),
        },
        extra: Some(stream::Extra::default().into()),
        ..Default::default()
    }
}

fn metadata_json(duration_seconds: f64, channels: u32) -> serde_json::Value {
    let mut value = serde_json::to_value(metadata()).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("duration".to_string(), serde_json::json!(duration_seconds));
        object.insert("channels".to_string(), serde_json::json!(channels));
        object.insert("timing_source".to_string(), serde_json::json!("native"));
    }
    value
}

fn stream_words_from_text(text: &str, start: f64, duration: f64) -> Vec<stream::Word> {
    let word_strs = split_words(text);
    let count = word_strs.len();

    if count == 0 {
        return Vec::new();
    }

    word_strs
        .into_iter()
        .enumerate()
        .map(|(index, word)| {
            let word_start = start + (index as f64 / count as f64) * duration;
            let word_end = if index + 1 == count {
                (start + duration - MIN_SYNTHETIC_DURATION_SECONDS)
                    .max(word_start + MIN_SYNTHETIC_DURATION_SECONDS)
            } else {
                start + ((index + 1) as f64 / count as f64) * duration
            };

            stream::Word {
                word: word.to_string(),
                start: word_start,
                end: word_end,
                confidence: 1.0,
                speaker: None,
                punctuated_word: Some(word.to_string()),
                language: None,
            }
        })
        .collect()
}

fn split_words(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .filter(|word| !word.is_empty())
        .collect()
}

fn normalize_transcript_text(text: &str) -> String {
    split_words(text).join(" ")
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use swift_rs::{Bool, SRData, SRString, swift};

    swift!(fn _apple_speech_availability() -> SRString);
    swift!(fn _apple_speech_supported_locales() -> SRString);
    swift!(fn _apple_speech_preferred_locales() -> SRString);
    swift!(fn _apple_speech_installed_locales() -> SRString);
    swift!(fn _apple_speech_download_state(locale: &SRString) -> SRString);
    swift!(fn _apple_speech_start_download(locale: &SRString) -> Bool);
    swift!(fn _apple_speech_release_locale(locale: &SRString) -> Bool);
    swift!(fn _apple_speech_transcribe_samples(samples: &SRData, locale: &SRString) -> SRString);
    swift!(fn _apple_speech_transcribe_audio_file(
        audio_path: &SRString,
        locale: &SRString
    ) -> SRString);
    swift!(fn _apple_speech_live_start(locale: &SRString) -> SRString);
    swift!(fn _apple_speech_live_append(source: &SRString, samples: &SRData) -> SRString);
    swift!(fn _apple_speech_live_finalize(source: &SRString) -> SRString);
    swift!(fn _apple_speech_live_stop() -> SRString);

    #[derive(serde::Deserialize)]
    struct LocalesPayload {
        locales: Vec<String>,
        error: Option<String>,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FileTranscriptionPayload {
        text: String,
        duration_seconds: f64,
        #[serde(default)]
        words: Vec<Word>,
        error: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct LiveAppendPayload {
        partials: Vec<LivePartial>,
        error: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct StatusPayload {
        running: bool,
        error: Option<String>,
    }

    pub(super) fn availability() -> Result<Availability> {
        let payload = unsafe { _apple_speech_availability() };
        Ok(serde_json::from_str(payload.as_str())?)
    }

    pub(super) fn supported_locales() -> Result<Vec<String>> {
        let payload = unsafe { _apple_speech_supported_locales() };
        let result: LocalesPayload = serde_json::from_str(payload.as_str())?;

        match result.error {
            Some(error) => Err(Error::Bridge(error)),
            None => Ok(result.locales),
        }
    }

    pub(super) fn preferred_locales() -> Result<Vec<String>> {
        let payload = unsafe { _apple_speech_preferred_locales() };
        let result: LocalesPayload = serde_json::from_str(payload.as_str())?;

        match result.error {
            Some(error) => Err(Error::Bridge(error)),
            None => Ok(result.locales),
        }
    }

    pub(super) fn installed_locales() -> Result<Vec<String>> {
        let payload = unsafe { _apple_speech_installed_locales() };
        let result: LocalesPayload = serde_json::from_str(payload.as_str())?;

        match result.error {
            Some(error) => Err(Error::Bridge(error)),
            None => Ok(result.locales),
        }
    }

    pub(super) fn download_state(locale: &str) -> Result<ModelDownloadState> {
        let locale = SRString::from(locale);
        let payload = unsafe { _apple_speech_download_state(&locale) };
        Ok(serde_json::from_str(payload.as_str())?)
    }

    pub(super) fn start_download(locale: &str) -> Result<()> {
        let locale_str = SRString::from(locale);
        if unsafe { _apple_speech_start_download(&locale_str) } {
            Ok(())
        } else {
            Err(Error::Bridge(format!(
                "failed to start Apple Speech asset download for {locale}"
            )))
        }
    }

    pub(super) fn release_locale(locale: &str) -> Result<()> {
        let locale_str = SRString::from(locale);
        if unsafe { _apple_speech_release_locale(&locale_str) } {
            Ok(())
        } else {
            Err(Error::Bridge(format!(
                "failed to release Apple Speech locale {locale}"
            )))
        }
    }

    pub(super) fn transcribe_samples(samples: &[f32], locale: &str) -> Result<FileTranscript> {
        let samples = floats_to_sr_data(samples);
        let locale = SRString::from(locale);
        let payload = unsafe { _apple_speech_transcribe_samples(&samples, &locale) };
        parse_file_transcript(payload.as_str())
    }

    pub(super) fn transcribe_file(path: &Path, locale: &str) -> Result<FileTranscript> {
        let audio_path = SRString::from(path.to_string_lossy().as_ref());
        let locale = SRString::from(locale);
        let payload = unsafe { _apple_speech_transcribe_audio_file(&audio_path, &locale) };
        parse_file_transcript(payload.as_str())
    }

    fn parse_file_transcript(payload: &str) -> Result<FileTranscript> {
        let result: FileTranscriptionPayload = serde_json::from_str(payload)?;

        if let Some(error) = result.error {
            return Err(Error::Bridge(error));
        }

        Ok(FileTranscript {
            text: result.text,
            duration_seconds: result.duration_seconds,
            words: result.words,
        })
    }

    pub(super) fn live_start(locale: &str) -> Result<()> {
        let locale = SRString::from(locale);
        let payload = unsafe { _apple_speech_live_start(&locale) };
        let result: StatusPayload = serde_json::from_str(payload.as_str())?;

        if result.running {
            Ok(())
        } else {
            Err(Error::Bridge(result.error.unwrap_or_else(|| {
                "failed to start Apple Speech live session".to_string()
            })))
        }
    }

    pub(super) fn live_append(
        source: TranscriptSource,
        samples: &[f32],
    ) -> Result<Vec<LivePartial>> {
        let source = SRString::from(source.as_str());
        let samples = floats_to_sr_data(samples);
        let payload = unsafe { _apple_speech_live_append(&source, &samples) };
        let result: LiveAppendPayload = serde_json::from_str(payload.as_str())?;

        if let Some(error) = result.error {
            return Err(Error::Bridge(error));
        }

        Ok(result.partials)
    }

    pub(super) fn live_finalize(source: TranscriptSource) -> Result<Vec<LivePartial>> {
        let source = SRString::from(source.as_str());
        let payload = unsafe { _apple_speech_live_finalize(&source) };
        let result: LiveAppendPayload = serde_json::from_str(payload.as_str())?;

        if let Some(error) = result.error {
            return Err(Error::Bridge(error));
        }

        Ok(result.partials)
    }

    pub(super) fn live_stop() -> Result<()> {
        let payload = unsafe { _apple_speech_live_stop() };
        let result: StatusPayload = serde_json::from_str(payload.as_str())?;

        match result.error {
            Some(error) => Err(Error::Bridge(error)),
            None => Ok(()),
        }
    }

    fn floats_to_sr_data(samples: &[f32]) -> SRData {
        let bytes = samples
            .iter()
            .flat_map(|sample| sample.to_bits().to_le_bytes())
            .collect::<Vec<_>>();
        SRData::from(bytes.as_slice())
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;

    pub(super) fn availability() -> Result<Availability> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn supported_locales() -> Result<Vec<String>> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn preferred_locales() -> Result<Vec<String>> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn installed_locales() -> Result<Vec<String>> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn download_state(_locale: &str) -> Result<ModelDownloadState> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn start_download(_locale: &str) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn release_locale(_locale: &str) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn transcribe_samples(_samples: &[f32], _locale: &str) -> Result<FileTranscript> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn transcribe_file(_path: &Path, _locale: &str) -> Result<FileTranscript> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn live_start(_locale: &str) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn live_append(
        _source: TranscriptSource,
        _samples: &[f32],
    ) -> Result<Vec<LivePartial>> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn live_finalize(_source: TranscriptSource) -> Result<Vec<LivePartial>> {
        Err(Error::UnsupportedPlatform)
    }

    pub(super) fn live_stop() -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
