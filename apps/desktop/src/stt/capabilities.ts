import type { LocalModel } from "@anlg/plugin-local-stt";
import {
  commands as listenerCommands,
  type TranscriptionMode,
} from "@anlg/plugin-transcription";

type LiveTranscriptionConfig = {
  languages: string[];
  transcriptionMode?: TranscriptionMode;
};

const SONIQO_PARAKEET_BATCH_LANGUAGE_CODES = new Set([
  "bg",
  "cs",
  "da",
  "de",
  "el",
  "en",
  "es",
  "et",
  "fi",
  "fr",
  "hr",
  "hu",
  "it",
  "lt",
  "lv",
  "mt",
  "nl",
  "pl",
  "pt",
  "ro",
  "ru",
  "sk",
  "sl",
  "sv",
  "uk",
]);
const SONIQO_STREAMING_LANGUAGE_CODES = SONIQO_PARAKEET_BATCH_LANGUAGE_CODES;

// Base codes of SpeechTranscriber.supportedLocales on macOS 26 (30 regional
// variants across these 10 languages).
const APPLE_SPEECH_LANGUAGE_CODES = new Set([
  "de",
  "en",
  "es",
  "fr",
  "it",
  "ja",
  "ko",
  "pt",
  "yue",
  "zh",
]);

/// Whether Apple Speech can transcribe the language at all. A language it supports but
/// the user has not added in System Settings is a different problem with a different fix.
export function canAppleSpeechTranscribe(language: string) {
  return APPLE_SPEECH_LANGUAGE_CODES.has(baseLanguageCode(language));
}

function liveLanguageCodesForModel(model: string | null | undefined) {
  return model === "apple-speech"
    ? APPLE_SPEECH_LANGUAGE_CODES
    : SONIQO_STREAMING_LANGUAGE_CODES;
}

export function isSupportedLocalSttModel(
  model?: string | null,
): model is LocalModel {
  return (
    typeof model === "string" &&
    (model.startsWith("soniqo-") ||
      model === "apple-speech" ||
      model.startsWith("am-") ||
      model.startsWith("Quantized"))
  );
}

export function isAnarlogCloudSttModel(
  provider?: string | null,
  model?: string | null,
) {
  return provider === "anarlog" && model === "cloud";
}

export function isOnDeviceSttModel(
  provider?: string | null,
  model?: string | null,
): model is LocalModel {
  if (!isSupportedLocalSttModel(model)) {
    return false;
  }

  if (provider === "soniqo") {
    return model.startsWith("soniqo-");
  }

  if (provider === "apple_speech" || provider === "apple-speech") {
    return model === "apple-speech";
  }

  return provider === "anarlog";
}

export function isDesktopLocalSttAvailable(
  currentPlatform: string,
  currentArch: string,
) {
  return currentPlatform === "macos" && currentArch === "aarch64";
}

export function getUnsupportedDesktopLocalSttRepair(
  currentPlatform: string,
  currentArch: string,
  provider: string | undefined,
  model: string | undefined,
  canUseCloud: boolean,
) {
  if (
    isDesktopLocalSttAvailable(currentPlatform, currentArch) ||
    !isOnDeviceSttModel(provider, model)
  ) {
    return null;
  }

  return canUseCloud
    ? { provider: "anarlog", model: "cloud" }
    : { provider: "", model: "" };
}

export function isConfiguredSttModel(
  provider?: string | null,
  model?: string | null,
) {
  if (!provider || !model) {
    return false;
  }

  if (provider === "anarlog") {
    return model === "cloud" || isSupportedLocalSttModel(model);
  }

  if (provider === "soniqo") {
    return model.startsWith("soniqo-");
  }

  if (provider === "apple_speech") {
    return model === "apple-speech";
  }

  return true;
}

export function isRealtimeLocalModel(model?: string | null) {
  return model === "soniqo-parakeet-streaming" || model === "apple-speech";
}

export function getSttModelTranscriptionMode(
  provider?: string | null,
  model?: string | null,
): TranscriptionMode | undefined {
  if (provider === "cohere" && model === "cohere-transcribe-03-2026") {
    return "batch";
  }

  if (
    provider === "groq" ||
    provider === "openrouter" ||
    provider === "siliconflow" ||
    provider === "together" ||
    provider === "zai" ||
    provider === "speechmatics" ||
    provider === "azure_speech" ||
    provider === "google_cloud" ||
    provider === "aws_transcribe" ||
    provider === "revai"
  ) {
    return "batch";
  }

  if (provider === "openai") {
    if (model === "gpt-live-transcribe") return "live";
    if (
      model === "gpt-transcribe" ||
      model === "gpt-4o-transcribe-diarize" ||
      model === "gpt-4o-transcribe" ||
      model === "gpt-4o-mini-transcribe" ||
      model === "whisper-1"
    ) {
      return "batch";
    }
  }

  if (provider === "assemblyai") {
    if (model === "universal-3-pro") return "batch";
    if (model === "u3-rt-pro") return "live";
  }

  if (provider === "elevenlabs") {
    if (model === "scribe_v2") return "batch";
    if (model === "scribe_v2_realtime") return "live";
  }

  if (provider === "mistral") {
    if (model === "voxtral-mini-2602" || model === "voxtral-mini-latest") {
      return "batch";
    }
    if (model === "voxtral-mini-transcribe-realtime-2602") return "live";
  }

  if (provider === "soniox") {
    if (model === "stt-async-v5" || model === "stt-async-v4") return "batch";
    if (
      model === "stt-rt-v5" ||
      model === "stt-rt-v4" ||
      model === "stt-v5" ||
      model === "stt-v4"
    ) {
      return "live";
    }
  }

  if (provider === "deepgram" && model?.startsWith("flux-")) {
    return "live";
  }

  if (provider === "gladia" && model === "solaria-3") {
    return "batch";
  }

  return undefined;
}

function baseLanguageCode(language: string) {
  return language.split(/[-_]/)[0]?.toLowerCase() ?? "";
}

function languageSupportProvider(provider: string) {
  if (provider === "custom" || provider === "cloudflare_workers_ai") {
    return "deepgram";
  }

  return provider === "apple_speech" ? "apple-speech" : provider;
}

export async function isSupportedLanguagesLive(
  provider: string,
  model: string | null | undefined,
  languages: readonly string[],
) {
  const result = await listenerCommands.isSupportedLanguagesLive(
    languageSupportProvider(provider),
    model ?? null,
    [...languages],
  );

  return result.status === "ok" ? result.data : true;
}

export async function isSupportedLanguagesBatch(
  provider: string,
  model: string | null | undefined,
  languages: readonly string[],
) {
  const result = await listenerCommands.isSupportedLanguagesBatch(
    languageSupportProvider(provider),
    model ?? null,
    [...languages],
  );

  return result.status === "ok" ? result.data : true;
}

export function getTranscriptionLanguages(
  mainLanguage: string | null | undefined,
  spokenLanguages: readonly string[] | null | undefined,
) {
  const seen = new Set<string>();
  const languages: string[] = [];

  for (const language of [mainLanguage, ...(spokenLanguages ?? [])]) {
    if (!language) {
      continue;
    }

    const baseCode = baseLanguageCode(language);
    if (!baseCode || seen.has(baseCode)) {
      continue;
    }

    seen.add(baseCode);
    languages.push(language);
  }

  return languages;
}

export function getOnDeviceTranscriptionConfig(
  model: string | null | undefined,
  languages: readonly string[],
): LiveTranscriptionConfig {
  if (!isRealtimeLocalModel(model)) {
    return {
      languages: [...languages],
      transcriptionMode: "batch",
    };
  }

  const liveLanguageCodes = liveLanguageCodesForModel(model);
  const supportedLiveLanguages = languages.filter((language) =>
    liveLanguageCodes.has(baseLanguageCode(language)),
  );

  if (languages.length > 0 && supportedLiveLanguages.length === 0) {
    return {
      languages: [],
      transcriptionMode: "live",
    };
  }

  return {
    languages:
      supportedLiveLanguages.length > 0
        ? [supportedLiveLanguages[0]]
        : [...languages],
    transcriptionMode: "live",
  };
}

export function getOnDeviceTranscriptionMode(
  model: string | null | undefined,
  languages: readonly string[] = [],
) {
  return getOnDeviceTranscriptionConfig(model, languages).transcriptionMode;
}

export async function getLiveTranscriptionConfig({
  provider,
  model,
  languages,
}: {
  provider?: string | null;
  model?: string | null;
  languages: readonly string[];
}): Promise<LiveTranscriptionConfig> {
  if (isOnDeviceSttModel(provider, model)) {
    return getOnDeviceTranscriptionConfig(model, languages);
  }

  const config = {
    languages: [...languages],
    transcriptionMode: getSttModelTranscriptionMode(provider, model),
  } satisfies LiveTranscriptionConfig;

  if (
    !provider ||
    config.transcriptionMode === "batch" ||
    languages.length <= 1
  ) {
    return config;
  }

  if (await isSupportedLanguagesLive(provider, model, languages)) {
    return config;
  }

  const primaryLanguage = languages[0];
  if (
    primaryLanguage &&
    (await isSupportedLanguagesLive(provider, model, [primaryLanguage]))
  ) {
    return {
      ...config,
      languages: [primaryLanguage],
    };
  }

  return config;
}

export async function isLiveTranscriptionSupported(
  provider?: string | null,
  model?: string | null,
) {
  if (!provider || !model) {
    return false;
  }

  return isSupportedLanguagesLive(provider, model, []);
}
