export type ProviderKind = "stt" | "llm";

export const TRANSCRIPTION_PROVIDERS = [
  {
    id: "anarlog",
    name: "Anarlog Pro",
    baseUrl: "",
    model: "cloud",
  },
  {
    id: "deepgram",
    name: "Deepgram",
    baseUrl: "https://api.deepgram.com/v1",
    model: "nova-3",
  },
  {
    id: "assemblyai",
    name: "AssemblyAI",
    baseUrl: "https://api.assemblyai.com",
    model: "universal-3-5-pro",
  },
  {
    id: "openai",
    name: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    model: "whisper-1",
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    model: "openai/gpt-transcribe",
  },
  {
    id: "dashscope",
    name: "Alibaba Cloud Model Studio",
    baseUrl: "https://dashscope-intl.aliyuncs.com",
    model: "qwen3-asr-flash-realtime",
  },
  {
    id: "zai",
    name: "Z.AI",
    baseUrl: "https://api.z.ai/api/paas/v4",
    model: "glm-asr-2512",
  },
  {
    id: "siliconflow",
    name: "SiliconFlow",
    baseUrl: "https://api.siliconflow.com/v1",
    model: "FunAudioLLM/SenseVoiceSmall",
  },
  {
    id: "google_generative_ai",
    name: "Google Gemini",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
    model: "gemini-3.5-transcribe",
  },
  {
    id: "google_cloud",
    name: "Google Cloud Speech-to-Text",
    baseUrl: "https://speech.googleapis.com/v1",
    model: "latest_long",
  },
  {
    id: "aws_transcribe",
    name: "Amazon Transcribe",
    baseUrl: "",
    model: "amazon-transcribe",
  },
  {
    id: "azure_speech",
    name: "Azure AI Speech",
    baseUrl: "",
    model: "fast-transcription",
  },
  {
    id: "elevenlabs",
    name: "ElevenLabs",
    baseUrl: "https://api.elevenlabs.io",
    model: "scribe_v2",
  },
  {
    id: "soniox",
    name: "Soniox",
    baseUrl: "https://api.soniox.com",
    model: "stt-rt-v5",
  },
  {
    id: "speechmatics",
    name: "Speechmatics",
    baseUrl: "https://eu1.asr.api.speechmatics.com/v2",
    model: "enhanced",
  },
  {
    id: "groq",
    name: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    model: "whisper-large-v3-turbo",
  },
  {
    id: "mistral",
    name: "Mistral",
    baseUrl: "https://api.mistral.ai/v1",
    model: "voxtral-mini-2602",
  },
  {
    id: "revai",
    name: "Rev AI",
    baseUrl: "https://api.rev.ai/speechtotext/v1",
    model: "machine",
  },
  {
    id: "gladia",
    name: "Gladia",
    baseUrl: "https://api.gladia.io",
    model: "solaria-3",
  },
  {
    id: "cartesia",
    name: "Cartesia",
    baseUrl: "https://api.cartesia.ai",
    model: "ink-2",
  },
  {
    id: "cloudflare_workers_ai",
    name: "Cloudflare Workers AI",
    baseUrl: "",
    model: "nova-3",
  },
  {
    id: "together",
    name: "Together AI",
    baseUrl: "https://api.together.xyz/v1",
    model: "openai/whisper-large-v3",
  },
  {
    id: "xai",
    name: "xAI",
    baseUrl: "https://api.x.ai/v1",
    model: "xai-stt",
  },
  {
    id: "smallestai",
    name: "Smallest AI",
    baseUrl: "https://api.smallest.ai",
    model: "pulse",
  },
  {
    id: "pyannote",
    name: "pyannoteAI",
    baseUrl: "https://api.pyannote.ai",
    model: "parakeet-tdt-0.6b-v3",
  },
  {
    id: "cohere",
    name: "Cohere",
    baseUrl: "https://api.cohere.com/v2",
    model: "cohere-transcribe-03-2026",
  },
  {
    id: "aquavoice",
    name: "AquaVoice",
    baseUrl: "https://api.aquavoice.com/v1",
    model: "avalon-v1.5",
  },
  {
    id: "custom",
    name: "Custom",
    baseUrl: "",
    model: "whisper-1",
  },
] as const;

export const SUMMARY_PROVIDERS = [
  {
    id: "anarlog",
    name: "Anarlog Pro",
    baseUrl: "",
    model: "",
  },
  {
    id: "openai",
    name: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    model: "",
  },
  {
    id: "anthropic",
    name: "Anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    model: "",
  },
  {
    id: "google_generative_ai",
    name: "Google Gemini",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
    model: "",
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    model: "",
  },
  {
    id: "moonshot",
    name: "Moonshot AI",
    baseUrl: "https://api.moonshot.ai/v1",
    model: "",
  },
  {
    id: "zai",
    name: "Z.AI",
    baseUrl: "https://api.z.ai/api/paas/v4",
    model: "",
  },
  {
    id: "deepseek",
    name: "DeepSeek",
    baseUrl: "https://api.deepseek.com",
    model: "",
  },
  {
    id: "alibaba_cloud",
    name: "Alibaba Cloud Model Studio",
    baseUrl: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
    model: "",
  },
  {
    id: "siliconflow",
    name: "SiliconFlow",
    baseUrl: "https://api.siliconflow.com/v1",
    model: "",
  },
  {
    id: "amazon_bedrock",
    name: "Amazon Bedrock",
    baseUrl: "",
    model: "",
  },
  {
    id: "azure_openai",
    name: "Azure OpenAI",
    baseUrl: "",
    model: "",
  },
  {
    id: "google_vertex_ai",
    name: "Google Vertex AI",
    baseUrl: "",
    model: "",
  },
  {
    id: "azure_ai",
    name: "Azure AI Foundry",
    baseUrl: "",
    model: "",
  },
  {
    id: "groq",
    name: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    model: "",
  },
  {
    id: "xai",
    name: "xAI",
    baseUrl: "https://api.x.ai/v1",
    model: "",
  },
  {
    id: "mistral",
    name: "Mistral",
    baseUrl: "https://api.mistral.ai/v1",
    model: "",
  },
  {
    id: "together",
    name: "Together AI",
    baseUrl: "https://api.together.xyz/v1",
    model: "",
  },
  {
    id: "cohere",
    name: "Cohere",
    baseUrl: "https://api.cohere.ai/compatibility/v1",
    model: "",
  },
  {
    id: "fireworks",
    name: "Fireworks AI",
    baseUrl: "https://api.fireworks.ai/inference/v1",
    model: "",
  },
  {
    id: "cloudflare_workers_ai",
    name: "Cloudflare Workers AI",
    baseUrl: "",
    model: "",
  },
  {
    id: "cerebras",
    name: "Cerebras",
    baseUrl: "https://api.cerebras.ai/v1",
    model: "",
  },
  {
    id: "custom",
    name: "Custom",
    baseUrl: "",
    model: "",
  },
] as const;

export type ProviderConfig = {
  provider: string;
  baseUrl: string;
  model: string;
};

export function providersFor(kind: ProviderKind) {
  return kind === "stt" ? TRANSCRIPTION_PROVIDERS : SUMMARY_PROVIDERS;
}

export function defaultProviderConfig(
  kind: ProviderKind,
  provider = "anarlog",
): ProviderConfig {
  const definition = providersFor(kind).find((entry) => entry.id === provider);
  if (!definition) throw new Error("Unsupported provider");
  return { provider, baseUrl: definition.baseUrl, model: definition.model };
}

export function validateProviderConfig(
  kind: ProviderKind,
  config: ProviderConfig,
): ProviderConfig {
  const connection = validateProviderConnection(kind, config);
  if (config.provider === "anarlog") return defaultProviderConfig(kind);
  const model = config.model.trim();
  if (!model || model.length > 200 || /[\r\n]/.test(model))
    throw new Error("Enter a model ID.");
  return { ...connection, model };
}

export function validateProviderConnection(
  kind: ProviderKind,
  config: { provider: string; baseUrl: string },
) {
  const definition = defaultProviderConfig(kind, config.provider);
  if (config.provider === "anarlog") return definition;
  let url: URL;
  try {
    url = new URL(definition.baseUrl || config.baseUrl.trim());
  } catch {
    throw new Error(
      "Enter an HTTPS base URL without credentials or query parameters.",
    );
  }
  if (
    url.protocol !== "https:" ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  ) {
    throw new Error(
      "Enter an HTTPS base URL without credentials or query parameters.",
    );
  }
  return {
    provider: config.provider,
    baseUrl: url.toString().replace(/\/+$/, ""),
  };
}

export function providerStorageKey(
  accountId: string | null,
  kind: ProviderKind,
  provider?: string,
): string {
  const account = accountId ?? "local";
  if (!/^[A-Za-z0-9_-]{1,128}$/.test(account))
    throw new Error("Invalid account");
  if (provider && !providersFor(kind).some((entry) => entry.id === provider))
    throw new Error("Unsupported provider");
  return `anarlog.provider.${account}.${kind}${provider ? `.${provider}` : ""}`;
}

export function validateProviderApiKey(value: string): string {
  const key = value.trim();
  if (!key) throw new Error("Enter an API key for this provider.");
  if (key.length > 8192 || /[\r\n]/.test(key))
    throw new Error("Enter a valid API key.");
  return key;
}

export function normalizeTranscriptionResponse(
  provider: string,
  payload: unknown,
): unknown {
  if (provider === "anarlog" || provider === "deepgram") return payload;
  if (!payload || typeof payload !== "object")
    throw new Error("Invalid transcription response");
  const data = payload as {
    text?: unknown;
    words?: unknown;
    results?: unknown;
  };
  if (data.results && typeof data.results === "object") return payload;
  if (typeof data.text !== "string")
    throw new Error("Invalid transcription response");
  return {
    results: {
      channels: [
        {
          alternatives: [
            {
              transcript: data.text,
              words: Array.isArray(data.words) ? data.words : [],
            },
          ],
        },
      ],
    },
  };
}
