//! `settings/ai/stt/shared.tsx`: the labels the Transcription page shows for
//! a provider's model ids (`displayModelId`) and the models OpenAI retires
//! (`isDeprecatedSttModel`).

/// `displayModelId`'s table, one row per id.
const MODEL_LABELS: &[(&str, &str)] = &[
    ("cloud", "Pro (Cloud)"),
    ("local-file", "whisper.cpp .bin"),
    ("nova-3", "Nova 3"),
    ("nova-3-general", "Nova 3"),
    ("nova-3-medical", "Nova 3 Medical"),
    ("flux-general-multi", "Flux General Multilingual"),
    ("flux-general-en", "Flux General English"),
    ("universal-3-5-pro-realtime", "Universal 3.5 Pro Realtime"),
    ("universal-3-5-pro", "Universal 3.5 Pro"),
    ("u3-rt-pro", "Universal 3 Pro Realtime"),
    ("universal-3-pro", "Universal 3 Pro"),
    ("universal", "Universal 3 Pro"),
    ("stt-v5", "Soniox 5"),
    ("stt-rt-v5", "Soniox 5"),
    ("stt-async-v5", "Soniox 5"),
    ("muse-voice-transcribe-1.0", "Muse Voice Transcribe"),
    ("stt-v4", "Soniox 4"),
    ("stt-rt-v4", "Soniox 4"),
    ("stt-async-v4", "Soniox 4"),
    ("stt-v3", "Soniox 3"),
    ("stt-rt-v3", "Soniox 3"),
    ("stt-async-v3", "Soniox 3"),
    ("solaria-1", "Solaria 1"),
    ("solaria-3", "Solaria 3"),
    ("scribe_v2_realtime", "Scribe V2 Realtime"),
    ("scribe_v2", "Scribe V2"),
    ("whisper-1", "Whisper 1"),
    ("gpt-live-transcribe", "GPT Live Transcribe"),
    ("gpt-transcribe", "GPT Transcribe"),
    ("ink-whisper", "Ink Whisper"),
    ("ink-2", "Ink 2"),
    ("gpt-4o-transcribe", "GPT-4o Transcribe"),
    ("gpt-4o-transcribe-diarize", "GPT-4o Transcribe Diarize"),
    ("gpt-4o-mini-transcribe", "GPT-4o mini Transcribe"),
    ("voxtral-mini-transcribe-realtime-2602", "Voxtral Realtime"),
    ("qwen3-asr-flash-realtime", "Qwen3 ASR Flash Realtime"),
    (
        "qwen3-asr-flash-realtime-2026-02-10",
        "Qwen3 ASR Flash Realtime (2026-02-10)",
    ),
    ("glm-asr-2512", "GLM ASR"),
    ("FunAudioLLM/SenseVoiceSmall", "SenseVoice Small"),
    ("TeleAI/TeleSpeechASR", "TeleSpeech ASR"),
    ("voxtral-mini-2602", "Voxtral Mini Transcribe 2"),
    ("avalon-v1.5", "Avalon 1.5"),
    ("avalon-v1-en", "Avalon V1"),
    ("cohere-transcribe-03-2026", "Cohere Transcribe"),
    (
        "cohere-transcribe-arabic-07-2026",
        "Cohere Transcribe Arabic",
    ),
    ("whisper-large-v3-turbo", "Whisper Large V3 Turbo"),
    ("whisper-v3-turbo", "Whisper V3 Turbo"),
    ("whisper-large-v3", "Whisper Large V3"),
    ("openai/whisper-large-v3", "Whisper Large V3"),
    ("xai-stt", "xAI Speech to Text"),
    ("pulse", "Pulse"),
    ("pulse-pro", "Pulse Pro"),
    ("gemini-3.5-transcribe-live", "3.5 Transcribe Live"),
    ("gemini-3.5-transcribe-live-preview", "3.5 Transcribe Live"),
    ("gemini-3.5-transcribe", "3.5 Transcribe"),
    ("gemini-3.5-transcribe-preview", "3.5 Transcribe"),
    ("enhanced", "Enhanced"),
    ("standard", "Standard"),
    ("fast-transcription", "Fast Transcription"),
    ("latest_long", "Latest Long"),
    ("amazon-transcribe", "Amazon Transcribe"),
    ("machine", "Machine Transcription"),
    ("apple-speech", "Apple Speech"),
    ("soniqo-parakeet-streaming", "Parakeet Streaming"),
    ("soniqo-parakeet-batch", "Parakeet Batch"),
    ("soniqo-omnilingual", "Omnilingual ASR"),
    ("soniqo-qwen3-small", "Qwen3 ASR 0.6B"),
    ("soniqo-qwen3-large", "Qwen3 ASR 1.7B"),
    ("parakeet-tdt-0.6b-v3", "Parakeet TDT 0.6B V3"),
    (
        "faster-whisper-large-v3-turbo",
        "Faster Whisper Large V3 Turbo",
    ),
];

/// `OPENROUTER_MODEL_LABELS`.
const OPENROUTER_MODEL_LABELS: &[(&str, &str)] = &[
    ("fish-audio/transcribe-1", "Transcribe 1"),
    ("x-ai/grok-stt-1.0", "Grok STT 1.0"),
    ("deepgram/nova-3", "Nova 3"),
    ("microsoft/mai-transcribe-1.5", "MAI Transcribe 1.5"),
    ("nvidia/parakeet-tdt-0.6b-v3", "Parakeet TDT 0.6B V3"),
    (
        "nvidia/nemotron-3-asr-streaming-0.6b",
        "Nemotron 3 ASR 0.6B",
    ),
    (
        "nvidia/nemotron-3.5-asr-streaming-0.6b",
        "Nemotron 3.5 ASR 0.6B",
    ),
    (
        "nvidia/nemotron-3.5-asr-streaming-multilingual-0.6b",
        "Nemotron 3.5 ASR Multilingual 0.6B",
    ),
    (
        "mistralai/voxtral-mini-transcribe",
        "Voxtral Mini Transcribe",
    ),
    ("mistralai/voxtral-mini-3b-2507", "Voxtral Mini 3B"),
    ("mistralai/voxtral-small-24b-2507-stt", "Voxtral Small 24B"),
    ("qwen/qwen3-asr-flash-2026-02-10", "Qwen3 ASR Flash"),
    ("qwen/qwen3-asr-0.6b", "Qwen3 ASR 0.6B"),
    ("qwen/qwen3-asr-1.7b", "Qwen3 ASR 1.7B"),
    ("google/chirp-3", "Chirp 3"),
];

/// `displayModelId`: the human name for a stored STT model id; an unknown
/// id shows as itself, `openai/` prefixes are looked up without the prefix.
pub fn display_model_id(model: &str) -> String {
    if let Some((_, label)) = MODEL_LABELS.iter().find(|(id, _)| *id == model) {
        return (*label).to_string();
    }
    if let Some((_, label)) = OPENROUTER_MODEL_LABELS.iter().find(|(id, _)| *id == model) {
        return (*label).to_string();
    }
    if let Some(rest) = model.strip_prefix("openai/") {
        return display_model_id(rest);
    }
    model.to_string()
}

/// `getLocalModelIcon`: the `AiIconSlot` art before a model's label — the
/// Anarlog mark for the cloud model, the Qwen / Meta / OpenAI / NVIDIA logos
/// for the model families that carry one. The Apple, GGML and Soniqo marks
/// belong to on-device rows that exist on Apple Silicon only.
pub fn model_icon(model: &str) -> Option<crate::ai_providers::Icon> {
    use crate::ai_providers::Icon;
    let value = model.to_lowercase();
    if value == "cloud" {
        return Some(Icon::Model("anarlog-icon.png"));
    }
    if value == "apple-speech" {
        return Some(Icon::Mono("brands/apple.svg", None));
    }
    if value.contains("qwen") {
        return Some(Icon::Model("model-icons/qwen-logo.svg"));
    }
    if value.contains("omnilingual") {
        return Some(Icon::Model("model-icons/meta-logo.svg"));
    }
    if value.contains("whisper") || value.contains("quantized") {
        return Some(Icon::Model("model-icons/openai-logo.svg"));
    }
    if value.contains("parakeet") {
        return Some(Icon::Model("model-icons/nvidia-logo.svg"));
    }
    None
}

/// `DEPRECATED_STT_MODELS`: OpenAI retires these on 2027-02-26; they stay
/// listed as the only OpenAI models with speaker labels and word timestamps.
pub fn is_deprecated_stt_model(provider: &str, model: &str) -> bool {
    provider == "openai" && matches!(model, "gpt-4o-transcribe-diarize" | "whisper-1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_display_model_id() {
        assert_eq!(display_model_id("nova-3"), "Nova 3");
        assert_eq!(display_model_id("nova-3-general"), "Nova 3");
        assert_eq!(display_model_id("stt-rt-v5"), "Soniox 5");
        assert_eq!(
            display_model_id("muse-voice-transcribe-1.0"),
            "Muse Voice Transcribe"
        );
        assert_eq!(
            display_model_id("gpt-4o-mini-transcribe"),
            "GPT-4o mini Transcribe"
        );
        assert_eq!(
            display_model_id("openai/whisper-large-v3"),
            "Whisper Large V3"
        );
        assert_eq!(display_model_id("openai/whisper-1"), "Whisper 1");
        assert_eq!(display_model_id("deepgram/nova-3"), "Nova 3");
        assert_eq!(
            display_model_id("gemini-3.5-transcribe-live-preview"),
            "3.5 Transcribe Live"
        );
        assert_eq!(display_model_id("unknown-model"), "unknown-model");
        assert_eq!(display_model_id("cloud"), "Pro (Cloud)");
        assert!(is_deprecated_stt_model("openai", "whisper-1"));
        assert!(!is_deprecated_stt_model("groq", "whisper-1"));
        assert_eq!(
            model_icon("whisper-large-v3-turbo"),
            Some(crate::ai_providers::Icon::Model(
                "model-icons/openai-logo.svg"
            ))
        );
        assert_eq!(
            model_icon("parakeet-tdt-0.6b-v3"),
            Some(crate::ai_providers::Icon::Model(
                "model-icons/nvidia-logo.svg"
            ))
        );
        assert_eq!(model_icon("nova-3"), None);
    }
}
