export type ProviderKind = "stt" | "llm";

export const TRANSCRIPTION_PROVIDERS = [
  { id: "anarlog", name: "Anarlog Pro", baseUrl: "", model: "cloud" },
  {
    id: "deepgram",
    name: "Deepgram",
    baseUrl: "https://api.deepgram.com/v1",
    model: "nova-3",
  },
  {
    id: "openai",
    name: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    model: "whisper-1",
  },
  {
    id: "groq",
    name: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    model: "whisper-large-v3-turbo",
  },
  { id: "custom", name: "Custom", baseUrl: "", model: "whisper-1" },
] as const;

export const SUMMARY_PROVIDERS = [
  { id: "anarlog", name: "Anarlog Pro", baseUrl: "", model: "" },
  {
    id: "openai",
    name: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    model: "",
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    model: "",
  },
  {
    id: "anthropic",
    name: "Anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    model: "",
  },
  {
    id: "groq",
    name: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    model: "",
  },
  { id: "custom", name: "Custom", baseUrl: "", model: "" },
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
  const definition = defaultProviderConfig(kind, config.provider);
  if (config.provider === "anarlog") return definition;
  const url = new URL(
    config.provider === "custom" ? config.baseUrl.trim() : definition.baseUrl,
  );
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
  const model = config.model.trim();
  if (!model || model.length > 200 || /[\r\n]/.test(model))
    throw new Error("Enter a model ID.");
  return {
    provider: config.provider,
    model,
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
  const data = payload as { text?: unknown; words?: unknown };
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
