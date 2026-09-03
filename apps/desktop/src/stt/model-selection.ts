type ModelEntry = {
  id: string;
  isDownloaded?: boolean;
};

type PreferredProviderModelOptions = {
  allowSavedModelWithoutChoices?: boolean;
  keepUnavailableSavedModel?: boolean;
};

const DEFAULT_EXTERNAL_STT_MODELS: Record<string, string> = {
  local_file: "local-file",
  deepgram: "nova-3-general",
  assemblyai: "universal-3-5-pro",
  openai: "gpt-live-transcribe",
  openrouter: "openai/gpt-transcribe",
  cartesia: "ink-2",
  cloudflare_workers_ai: "nova-3",
  gladia: "solaria-1",
  soniox: "stt-rt-v5",
  elevenlabs: "scribe_v2",
  mistral: "voxtral-mini-2602",
  pyannote: "parakeet-tdt-0.6b-v3",
  aquavoice: "avalon-v1.5",
  cohere: "cohere-transcribe-03-2026",
  dashscope: "qwen3-asr-flash-realtime",
  zai: "glm-asr-2512",
  siliconflow: "FunAudioLLM/SenseVoiceSmall",
  fireworks: "whisper-v3-turbo",
  groq: "whisper-large-v3-turbo",
  xai: "xai-stt",
  smallestai: "pulse",
  together: "openai/whisper-large-v3",
  speechmatics: "enhanced",
  azure_speech: "fast-transcription",
  google_cloud: "latest_long",
  google_generative_ai: "gemini-3.5-transcribe-live",
  aws_transcribe: "amazon-transcribe",
  revai: "machine",
};

export function getDefaultSttModel(provider?: string | null) {
  return provider ? DEFAULT_EXTERNAL_STT_MODELS[provider] : undefined;
}

export function normalizeStoredSttModel(
  provider: string | undefined,
  model: string | undefined,
) {
  if (provider === "assemblyai") {
    if (model === "universal" || model === "universal-3-pro") {
      return "universal-3-5-pro";
    }
    if (model === "u3-rt-pro") {
      return "universal-3-5-pro-realtime";
    }
  }

  if (provider === "aquavoice" && model === "avalon-v1-en") {
    return "avalon-v1.5";
  }

  // Soniox removed v3 and v4 (v4 survives only as a server-side alias of v5).
  if (provider === "soniox" && model?.match(/^stt-(?:async-|rt-)?v[3-5]$/)) {
    return "stt-rt-v5";
  }

  // OpenAI retires the gpt-4o transcribe pair on 2027-02-26; gpt-transcribe
  // is the documented replacement. The diarize variant has no successor yet.
  if (
    provider === "openai" &&
    (model === "gpt-4o-transcribe" || model === "gpt-4o-mini-transcribe")
  ) {
    return "gpt-transcribe";
  }

  if (
    provider === "openrouter" &&
    (model === "openai/gpt-4o-transcribe" ||
      model === "openai/gpt-4o-mini-transcribe")
  ) {
    return "openai/gpt-transcribe";
  }

  return model;
}

export function normalizeStoredSttSelection(
  provider: string | undefined,
  model: string | undefined,
) {
  const normalizedModel = normalizeStoredSttModel(provider, model);

  if (provider === "anarlog" && normalizedModel?.startsWith("soniqo-")) {
    return { provider: "soniqo", model: normalizedModel };
  }

  if (provider === "anarlog" && normalizedModel === "apple-speech") {
    return { provider: "apple_speech", model: normalizedModel };
  }

  return { provider, model: normalizedModel };
}

const normalizeSavedModel = (
  savedModel: string | undefined,
  models: ModelEntry[],
) => {
  if (
    (savedModel === "universal" || savedModel === "universal-3-pro") &&
    models.some((model) => model.id === "universal-3-5-pro")
  ) {
    return "universal-3-5-pro";
  }

  if (
    savedModel === "u3-rt-pro" &&
    models.some((model) => model.id === "universal-3-5-pro-realtime")
  ) {
    return "universal-3-5-pro-realtime";
  }

  if (
    savedModel === "avalon-v1-en" &&
    models.some((model) => model.id === "avalon-v1.5")
  ) {
    return "avalon-v1.5";
  }

  if (
    savedModel?.match(/^stt-(?:async-|rt-)?v[3-5]$/) &&
    models.some((model) => model.id === "stt-rt-v5")
  ) {
    return "stt-rt-v5";
  }

  if (
    (savedModel === "gpt-4o-transcribe" ||
      savedModel === "gpt-4o-mini-transcribe") &&
    models.some((model) => model.id === "gpt-transcribe")
  ) {
    return "gpt-transcribe";
  }

  if (
    (savedModel === "openai/gpt-4o-transcribe" ||
      savedModel === "openai/gpt-4o-mini-transcribe") &&
    models.some((model) => model.id === "openai/gpt-transcribe")
  ) {
    return "openai/gpt-transcribe";
  }

  return savedModel;
};

export function getPreferredProviderModel(
  savedModel: string | undefined,
  models: ModelEntry[],
  options?: PreferredProviderModelOptions,
) {
  const normalizedSavedModel = normalizeSavedModel(savedModel, models);
  const selectableModels = models.filter((model) => model.isDownloaded ?? true);

  if (
    options?.keepUnavailableSavedModel &&
    normalizedSavedModel &&
    models.some((model) => model.id === normalizedSavedModel)
  ) {
    return normalizedSavedModel;
  }

  if (
    normalizedSavedModel &&
    selectableModels.some((model) => model.id === normalizedSavedModel)
  ) {
    return normalizedSavedModel;
  }

  if (selectableModels.length > 0) {
    return selectableModels[0].id;
  }

  if (options?.allowSavedModelWithoutChoices) {
    return normalizedSavedModel ?? "";
  }

  return "";
}
