import {
  commands as listenerCommands,
  type TranscriptionMode,
} from "@hypr/plugin-transcription";

type LiveTranscriptionConfig = {
  languages: string[];
  transcriptionMode?: TranscriptionMode;
};

const SONIQO_STREAMING_LANGUAGE_CODES = new Set(["en"]);

export function isRealtimeLocalModel(model?: string | null) {
  return model === "soniqo-parakeet-streaming";
}

function baseLanguageCode(language: string) {
  return language.split(/[-_]/)[0]?.toLowerCase() ?? "";
}

function languageSupportProvider(provider: string) {
  return provider === "custom" ? "deepgram" : provider;
}

async function isSupportedLanguagesLive(
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

  const primaryLanguage = languages[0];
  const primaryLanguageSupported =
    !primaryLanguage ||
    SONIQO_STREAMING_LANGUAGE_CODES.has(baseLanguageCode(primaryLanguage));

  // The Soniqo streaming session API does not accept a language hint, so keep
  // non-English primary languages on batch instead of relying on live auto-detect.
  if (!primaryLanguageSupported) {
    return {
      languages: [...languages],
      transcriptionMode: "batch",
    };
  }

  const supportedLiveLanguages = languages.filter((language) =>
    SONIQO_STREAMING_LANGUAGE_CODES.has(baseLanguageCode(language)),
  );

  if (languages.length > 0 && supportedLiveLanguages.length === 0) {
    return {
      languages: [...languages],
      transcriptionMode: "batch",
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
  if (provider === "hyprnote" && model !== "cloud") {
    return getOnDeviceTranscriptionConfig(model, languages);
  }

  const config = {
    languages: [...languages],
    transcriptionMode: undefined as TranscriptionMode | undefined,
  } satisfies LiveTranscriptionConfig;

  if (!provider || languages.length <= 1) {
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
