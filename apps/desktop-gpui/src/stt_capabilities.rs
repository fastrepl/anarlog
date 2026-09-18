//! `stt/capabilities.ts`: which transcription mode a provider's model runs
//! in and the languages a live session can carry, as `useStartListening`
//! resolves them through `getLiveTranscriptionConfig` before a capture.

use anlg_listener_core::TranscriptionMode;

/// `LiveTranscriptionConfig`.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveTranscriptionConfig {
    pub languages: Vec<anlg_language::Language>,
    /// Languages the live session drops because the provider cannot carry
    /// them alongside the primary one; the capture warns about them.
    pub omitted_languages: Vec<anlg_language::Language>,
    pub mode: TranscriptionMode,
}

/// `SONIQO_PARAKEET_BATCH_LANGUAGE_CODES` (the streaming set is the same).
const SONIQO_STREAMING_LANGUAGE_CODES: &[&str] = &[
    "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv", "mt",
    "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
];

/// Base codes of `SpeechTranscriber.supportedLocales` on macOS 26.
const APPLE_SPEECH_LANGUAGE_CODES: &[&str] =
    &["de", "en", "es", "fr", "it", "ja", "ko", "pt", "yue", "zh"];

fn base_language_code(language: &anlg_language::Language) -> String {
    let code = language.to_string();
    code.split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

/// `isRealtimeLocalModel`
pub fn is_realtime_local_model(model: &str) -> bool {
    model == "soniqo-parakeet-streaming" || model == "apple-speech"
}

fn live_language_codes_for_model(model: &str) -> &'static [&'static str] {
    if model == "apple-speech" {
        APPLE_SPEECH_LANGUAGE_CODES
    } else {
        SONIQO_STREAMING_LANGUAGE_CODES
    }
}

/// `getSttModelTranscriptionMode`: the mode a cloud provider's model is
/// known to run in; `None` when the frontend leaves it to the engine.
pub fn model_transcription_mode(provider: &str, model: &str) -> Option<TranscriptionMode> {
    use TranscriptionMode::{Batch, Live};
    if crate::db::is_local_file_stt_model(provider, model) {
        return Some(Batch);
    }
    if provider == "smallestai" && model == "pulse-pro" {
        return Some(Batch);
    }
    if matches!(
        provider,
        "groq"
            | "openrouter"
            | "siliconflow"
            | "together"
            | "zai"
            | "speechmatics"
            | "azure_speech"
            | "google_cloud"
            | "aws_transcribe"
            | "revai"
            | "pyannote"
            | "aquavoice"
            | "cohere"
    ) {
        return Some(Batch);
    }
    match provider {
        "google_generative_ai" => {
            if model.contains("transcribe-live") {
                return Some(Live);
            }
            if !model.is_empty() {
                return Some(Batch);
            }
        }
        "openai" => {
            if model == "gpt-live-transcribe" {
                return Some(Live);
            }
            if matches!(
                model,
                "gpt-transcribe"
                    | "gpt-4o-transcribe-diarize"
                    | "gpt-4o-transcribe"
                    | "gpt-4o-mini-transcribe"
                    | "whisper-1"
            ) {
                return Some(Batch);
            }
        }
        "assemblyai" => {
            if matches!(model, "universal-3-pro" | "universal-3-5-pro") {
                return Some(Batch);
            }
            if matches!(model, "u3-rt-pro" | "universal-3-5-pro-realtime") {
                return Some(Live);
            }
        }
        "elevenlabs" => {
            if model == "scribe_v2" {
                return Some(Batch);
            }
            if model == "scribe_v2_realtime" {
                return Some(Live);
            }
        }
        "mistral" => {
            if matches!(model, "voxtral-mini-2602" | "voxtral-mini-latest") {
                return Some(Batch);
            }
            if model == "voxtral-mini-transcribe-realtime-2602" {
                return Some(Live);
            }
        }
        "soniox" => {
            if matches!(model, "stt-async-v5" | "stt-async-v4") {
                return Some(Batch);
            }
            if matches!(model, "stt-rt-v5" | "stt-rt-v4" | "stt-v5" | "stt-v4") {
                return Some(Live);
            }
        }
        "deepgram" if model.starts_with("flux-") => return Some(Live),
        "gladia" if model == "solaria-3" => return Some(Batch),
        _ => {}
    }
    None
}

/// `getOnDeviceTranscriptionConfig`.
fn on_device_config(model: &str, languages: &[anlg_language::Language]) -> LiveTranscriptionConfig {
    if !is_realtime_local_model(model) {
        return LiveTranscriptionConfig {
            languages: languages.to_vec(),
            omitted_languages: Vec::new(),
            mode: TranscriptionMode::Batch,
        };
    }
    let codes = live_language_codes_for_model(model);
    let supported: Vec<_> = languages
        .iter()
        .filter(|language| codes.contains(&base_language_code(language).as_str()))
        .cloned()
        .collect();
    let languages = if !languages.is_empty() && supported.is_empty() {
        Vec::new()
    } else if let Some(first) = supported.first() {
        vec![first.clone()]
    } else {
        languages.to_vec()
    };
    LiveTranscriptionConfig {
        languages,
        omitted_languages: Vec::new(),
        mode: TranscriptionMode::Live,
    }
}

/// `languageSupportProvider`: the adapter the language tables are keyed by.
fn language_support_provider(provider: &str) -> &str {
    match provider {
        "local_file" => "anarlog",
        "custom" | "cloudflare_workers_ai" => "deepgram",
        "apple_speech" => "apple-speech",
        other => other,
    }
}

/// `isSupportedLanguagesLive`: a lookup failure counts as supported, like
/// the frontend's `result.status === "ok" ? result.data : true`.
fn supports_languages_live(
    provider: &str,
    model: &str,
    languages: &[anlg_language::Language],
) -> bool {
    anlg_listener2_core::is_supported_languages_live(
        language_support_provider(provider),
        (!model.is_empty()).then_some(model),
        languages,
    )
    .unwrap_or(true)
}

/// `getLiveTranscriptionConfig`: the mode for the selected model and the
/// languages the live session keeps — every requested language when the
/// provider carries them together, the primary one alone (the rest
/// `omitted`) when it carries only that, and the full list otherwise.
pub fn live_transcription_config(
    provider: Option<&str>,
    model: Option<&str>,
    languages: &[anlg_language::Language],
) -> LiveTranscriptionConfig {
    let provider_id = provider.unwrap_or_default();
    let model_id = model.unwrap_or_default();
    if crate::db::is_local_file_stt_model(provider_id, model_id) {
        return LiveTranscriptionConfig {
            languages: languages.to_vec(),
            omitted_languages: Vec::new(),
            mode: TranscriptionMode::Batch,
        };
    }
    if crate::db::is_on_device_stt_model(provider_id, model_id) {
        return on_device_config(model_id, languages);
    }
    // `transcriptionMode: undefined` leaves the engine's default, live.
    let mode = model_transcription_mode(provider_id, model_id).unwrap_or(TranscriptionMode::Live);
    let config = LiveTranscriptionConfig {
        languages: languages.to_vec(),
        omitted_languages: Vec::new(),
        mode,
    };
    if provider.is_none_or(str::is_empty)
        || mode == TranscriptionMode::Batch
        || languages.len() <= 1
    {
        return config;
    }
    if supports_languages_live(provider_id, model_id, languages) {
        return config;
    }
    if let Some(primary) = languages.first()
        && supports_languages_live(provider_id, model_id, std::slice::from_ref(primary))
    {
        return LiveTranscriptionConfig {
            languages: vec![primary.clone()],
            omitted_languages: languages[1..].to_vec(),
            mode,
        };
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    fn languages(codes: &[&str]) -> Vec<anlg_language::Language> {
        codes.iter().map(|code| code.parse().unwrap()).collect()
    }

    #[test]
    fn model_modes_follow_the_capability_table() {
        use TranscriptionMode::{Batch, Live};
        assert_eq!(model_transcription_mode("openai", "whisper-1"), Some(Batch));
        assert_eq!(
            model_transcription_mode("openai", "gpt-live-transcribe"),
            Some(Live)
        );
        assert_eq!(
            model_transcription_mode("groq", "whisper-large-v3"),
            Some(Batch)
        );
        assert_eq!(
            model_transcription_mode("deepgram", "flux-general-en"),
            Some(Live)
        );
        assert_eq!(model_transcription_mode("deepgram", "nova-3"), None);
        assert_eq!(
            model_transcription_mode("soniox", "stt-async-v5"),
            Some(Batch)
        );
        assert_eq!(model_transcription_mode("soniox", "stt-rt-v5"), Some(Live));
        assert_eq!(
            model_transcription_mode("google_generative_ai", "gemini-3.5-transcribe-live"),
            Some(Live)
        );
        assert_eq!(
            model_transcription_mode("google_generative_ai", "gemini-3.5-transcribe"),
            Some(Batch)
        );
        assert_eq!(
            model_transcription_mode("local_file", "local-file"),
            Some(Batch)
        );
        assert_eq!(model_transcription_mode("smallestai", "pulse"), None);
        assert_eq!(
            model_transcription_mode("smallestai", "pulse-pro"),
            Some(Batch)
        );
    }

    #[test]
    fn live_config_keeps_batch_models_out_of_the_live_session() {
        let config =
            live_transcription_config(Some("openai"), Some("whisper-1"), &languages(&["en", "ko"]));
        assert_eq!(config.mode, TranscriptionMode::Batch);
        assert_eq!(config.languages, languages(&["en", "ko"]));
        assert!(config.omitted_languages.is_empty());
        // No provider: the engine's live default with every language.
        let none = live_transcription_config(None, None, &languages(&["en", "ko"]));
        assert_eq!(none.mode, TranscriptionMode::Live);
        assert_eq!(none.languages, languages(&["en", "ko"]));
        // One language never consults the support tables.
        let single =
            live_transcription_config(Some("deepgram"), Some("nova-3"), &languages(&["en"]));
        assert_eq!(single.mode, TranscriptionMode::Live);
        assert_eq!(single.languages, languages(&["en"]));
    }

    #[test]
    fn live_config_narrows_to_the_primary_language_when_needed() {
        // Deepgram's nova-3 carries English with Spanish live (multilingual)
        // but not with Korean: the primary stays and Korean is omitted.
        let both =
            live_transcription_config(Some("deepgram"), Some("nova-3"), &languages(&["en", "es"]));
        assert_eq!(both.languages, languages(&["en", "es"]));
        assert!(both.omitted_languages.is_empty());
        let narrowed =
            live_transcription_config(Some("deepgram"), Some("nova-3"), &languages(&["en", "ko"]));
        assert_eq!(narrowed.mode, TranscriptionMode::Live);
        assert_eq!(narrowed.languages, languages(&["en"]));
        assert_eq!(narrowed.omitted_languages, languages(&["ko"]));
        // The `custom` provider maps onto Deepgram's tables like the frontend.
        let custom =
            live_transcription_config(Some("custom"), Some("nova-3"), &languages(&["en", "es"]));
        assert_eq!(custom.languages.len(), 2);
    }

    #[test]
    fn on_device_models_take_their_own_language_sets() {
        let soniqo = live_transcription_config(
            Some("soniqo"),
            Some("soniqo-parakeet-streaming"),
            &languages(&["ko", "en"]),
        );
        assert_eq!(soniqo.mode, TranscriptionMode::Live);
        assert_eq!(soniqo.languages, languages(&["en"]));
        let unsupported = live_transcription_config(
            Some("soniqo"),
            Some("soniqo-parakeet-streaming"),
            &languages(&["ko"]),
        );
        assert!(unsupported.languages.is_empty());
        let batch = live_transcription_config(
            Some("soniqo"),
            Some("soniqo-parakeet-v3"),
            &languages(&["ko"]),
        );
        assert_eq!(batch.mode, TranscriptionMode::Batch);
        assert_eq!(batch.languages, languages(&["ko"]));
    }
}
