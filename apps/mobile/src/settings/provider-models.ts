import { fetch } from "expo/fetch";

import { readProviderKey, readProviderSetup } from "./providers";
import {
  isOAuthSubscriptionProvider,
  validateProviderConnection,
  type ProviderConfig,
  type ProviderKind,
} from "./providers-model";
import { resolveSubscriptionCredential } from "./subscription-access";
import { listSubscriptionModels } from "./subscription-oauth";

const LISTABLE_REMOTE_STT_PROVIDERS = new Set([
  "deepgram",
  "soniox",
  "elevenlabs",
  "openai",
  "groq",
  "mistral",
  "fireworks",
  "together",
  "openrouter",
  "siliconflow",
  "dashscope",
  "google_generative_ai",
  "cloudflare_workers_ai",
]);

export function isListableRemoteSttProvider(provider: string) {
  return LISTABLE_REMOTE_STT_PROVIDERS.has(provider);
}

export async function discoverProviderModels(
  account: string | null,
  config: ProviderConfig,
  signal: AbortSignal,
  kind: ProviderKind = "llm",
): Promise<string[]> {
  signal.throwIfAborted();
  const connection = validateProviderConnection(kind, config);
  const saved = await readProviderSetup(account, kind, config.provider);
  if (saved.baseUrl !== connection.baseUrl)
    throw new Error("Provider connection changed. Reload models.");
  if (kind === "llm" && isOAuthSubscriptionProvider(config.provider)) {
    const credential = await resolveSubscriptionCredential(
      account,
      config.provider,
    );
    signal.throwIfAborted();
    return listSubscriptionModels(config.provider, credential, fetch, signal);
  }
  const apiKey = await readProviderKey(account, kind, config.provider);
  if (!apiKey) throw new Error("Add an API key to load models.");
  signal.throwIfAborted();
  const controller = new AbortController();
  const abort = () => controller.abort();
  signal.addEventListener("abort", abort, { once: true });
  const timer = setTimeout(abort, 8_000);
  try {
    if (kind === "stt") {
      return await discoverSttModels(
        config.provider,
        connection.baseUrl,
        apiKey,
        controller.signal,
      );
    }

    const normalizedBase = connection.baseUrl.replace(/\/+$/, "");
    let url = `${normalizedBase}/models`;
    const headers: Record<string, string> = {};
    switch (config.provider) {
      case "venice":
        url += "?type=text";
        headers.Authorization = `Bearer ${apiKey}`;
        break;
      case "anthropic":
        headers["x-api-key"] = apiKey;
        headers["anthropic-version"] = "2023-06-01";
        break;
      case "google_generative_ai":
        headers["x-goog-api-key"] = apiKey;
        break;
      case "azure_openai": {
        const base = normalizedBase.replace(/\/openai(?:\/v1)?$/, "");
        url = `${base}/openai/models?api-version=2024-10-21`;
        headers["api-key"] = apiKey;
        break;
      }
      case "azure_ai":
        headers["api-key"] = apiKey;
        break;
      case "google_vertex_ai": {
        const vertexBase = normalizedBase.replace(
          /\/(?:chat\/completions|endpoints)$/i,
          "",
        );
        url = `${vertexBase}/models`;
        headers.Authorization = `Bearer ${apiKey}`;
        break;
      }
      default:
        headers.Authorization = `Bearer ${apiKey}`;
    }

    const response = await fetch(url, {
      headers,
      signal: controller.signal,
      redirect: "error",
    });
    if (response.ok) {
      const parsed = parseProviderModels(
        config.provider,
        JSON.parse(await readModels(response)),
      );
      if (parsed.length > 0 || config.provider !== "cloudflare_workers_ai") {
        return parsed;
      }
    } else {
      await response.body?.cancel();
      if (
        config.provider !== "cloudflare_workers_ai" &&
        config.provider !== "google_vertex_ai"
      ) {
        throw new Error("Model request failed.");
      }
    }

    if (config.provider === "cloudflare_workers_ai") {
      const searchBase = normalizedBase.replace(/\/v1$/i, "");
      const searchRes = await fetch(
        `${searchBase}/models/search?task=Text%20Generation`,
        {
          headers,
          signal: controller.signal,
          redirect: "error",
        },
      );
      if (!searchRes.ok) {
        await searchRes.body?.cancel();
        throw new Error("Model request failed.");
      }
      return parseProviderModels(
        config.provider,
        JSON.parse(await readModels(searchRes)),
      );
    }

    const publisherRes = await fetch(
      `${normalizedBase.replace(/\/(?:chat\/completions|endpoints)$/i, "")}/publishers/google/models`,
      {
        headers,
        signal: controller.signal,
        redirect: "error",
      },
    );
    if (!publisherRes.ok) {
      await publisherRes.body?.cancel();
      throw new Error("Model request failed.");
    }
    return parseProviderModels(
      config.provider,
      JSON.parse(await readModels(publisherRes)),
    );
  } catch {
    throw new Error("Couldn’t load models. Retry or enter a model ID.");
  } finally {
    clearTimeout(timer);
    signal.removeEventListener("abort", abort);
  }
}

async function discoverSttModels(
  provider: string,
  baseUrl: string,
  apiKey: string,
  signal: AbortSignal,
): Promise<string[]> {
  const normalizedBase = baseUrl.replace(/\/+$/, "");
  const origin = new URL(normalizedBase).origin;
  let url = `${normalizedBase}/models`;
  const headers: Record<string, string> = {};

  switch (provider) {
    case "deepgram":
      url = normalizedBase.endsWith("/v1")
        ? `${normalizedBase}/models`
        : `${normalizedBase}/v1/models`;
      headers.Authorization = `Token ${apiKey}`;
      break;
    case "soniox":
      url = `${normalizedBase.replace(/\/v1$/, "")}/v1/models`;
      headers.Authorization = `Bearer ${apiKey}`;
      break;
    case "elevenlabs":
      url = `${normalizedBase.replace(/\/v1$/, "")}/v1/models`;
      headers["xi-api-key"] = apiKey;
      break;
    case "fireworks":
      url = `${origin}/inference/v1/models`;
      headers.Authorization = `Bearer ${apiKey}`;
      break;
    case "siliconflow":
      url = `${normalizedBase}/models?sub_type=audio`;
      headers.Authorization = `Bearer ${apiKey}`;
      break;
    case "dashscope":
      url = `${normalizedBase.replace(/\/compatible-mode\/v1$/, "")}/compatible-mode/v1/models`;
      headers.Authorization = `Bearer ${apiKey}`;
      break;
    case "google_generative_ai":
      headers["x-goog-api-key"] = apiKey;
      break;
    case "cloudflare_workers_ai":
      url = `${normalizedBase.replace(/\/v1$/i, "")}/models/search?task=Automatic%20Speech%20Recognition`;
      headers.Authorization = `Bearer ${apiKey}`;
      break;
    default:
      headers.Authorization = `Bearer ${apiKey}`;
      break;
  }

  const response = await fetch(url, {
    headers,
    signal,
    redirect: "error",
  });
  if (!response.ok) {
    await response.body?.cancel();
    throw new Error("Model request failed.");
  }
  return parseSttProviderModels(provider, JSON.parse(await readModels(response)));
}

async function readModels(response: Response) {
  const limit = 8 * 1024 * 1024;
  if (Number(response.headers.get("content-length")) > limit) {
    await response.body?.cancel();
    throw new Error("Model list is too large.");
  }
  const reader = response.body?.getReader();
  if (!reader) throw new Error("Empty model response.");
  const decoder = new TextDecoder();
  let bytes = 0;
  let text = "";
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) return text + decoder.decode();
      bytes += value.byteLength;
      if (bytes > limit) {
        await reader.cancel();
        throw new Error("Model list is too large.");
      }
      text += decoder.decode(value, { stream: true });
    }
  } finally {
    reader.releaseLock();
  }
}

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
}

export function parseSttProviderModels(
  provider: string,
  response: unknown,
): string[] {
  const body = record(response);
  let items: unknown[] | undefined;

  if (provider === "deepgram") {
    items = Array.isArray(body.stt) ? body.stt : undefined;
  } else if (provider === "elevenlabs") {
    items = Array.isArray(response)
      ? response
      : Array.isArray(body.models)
        ? body.models
        : undefined;
  } else if (provider === "soniox" || provider === "google_generative_ai") {
    items = Array.isArray(body.models) ? body.models : undefined;
  } else if (provider === "cloudflare_workers_ai") {
    items = Array.isArray(body.result)
      ? body.result
      : Array.isArray(body.data)
        ? body.data
        : undefined;
  } else {
    items = Array.isArray(body.data)
      ? body.data
      : Array.isArray(response)
        ? response
        : undefined;
  }

  if (!Array.isArray(items)) throw new Error("Invalid model list.");

  const models = items.flatMap((entry: unknown) => {
    const item = record(entry);
    let raw: unknown;
    switch (provider) {
      case "deepgram":
        raw = item.canonical_name || item.name;
        break;
      case "elevenlabs":
        raw = item.model_id;
        break;
      case "google_generative_ai":
        raw =
          typeof item.name === "string"
            ? item.name.replace(/^models\//, "")
            : item.name;
        break;
      case "cloudflare_workers_ai":
        raw = item.name || item.id;
        break;
      default:
        raw = item.id;
        break;
    }
    if (typeof raw !== "string") return [];
    const id = raw.trim();
    if (!id || id.length > 200 || /[\r\n]/.test(id)) return [];

    switch (provider) {
      case "elevenlabs":
        return item.can_do_text_to_speech === true &&
          !id.toLowerCase().includes("scribe")
          ? []
          : [id];
      case "openai":
        return /^(whisper|.*transcribe)/i.test(id) ? [id] : [];
      case "groq":
        return /whisper|asr|transcribe/i.test(id) ? [id] : [];
      case "mistral":
        return /voxtral|transcribe|asr|speech/i.test(id) ? [id] : [];
      case "fireworks":
        return /whisper|asr|transcribe|speech|audio/i.test(id) ? [id] : [];
      case "together":
        return item.type === "audio" ||
          /whisper|asr|transcribe|speech/i.test(id)
          ? [id]
          : [];
      case "openrouter": {
        const inputs = record(item.architecture).input_modalities;
        return Array.isArray(inputs) && inputs.includes("audio") ? [id] : [];
      }
      case "dashscope":
        return /asr|transcribe|sensevoice|paraformer|whisper|audio/i.test(id)
          ? [id]
          : [];
      case "google_generative_ai":
        return /gemini.*(transcribe|live)/i.test(id) ? [id] : [];
      default:
        return [id];
    }
  });

  return [...new Set(models)].sort((a, b) =>
    a.localeCompare(b, undefined, { numeric: true }),
  );
}

export function parseProviderModels(
  provider: string,
  response: unknown,
): string[] {
  const body = record(response);
  const data =
    provider === "google_vertex_ai"
      ? (body.models ?? body.publisherModels ?? body.data)
      : provider === "cloudflare_workers_ai"
        ? (body.data ?? body.result)
        : (body.data ?? body.models);
  if (!Array.isArray(data)) throw new Error("Invalid model list.");
  const models = data.flatMap((entry: unknown) => {
    const model = record(entry);
    if (
      provider === "venice" &&
      (model.type !== "text" || record(model.model_spec).offline === true)
    )
      return [];
    if (provider === "cloudflare_workers_ai") {
      const taskName = record(model.task).name;
      if (
        typeof taskName === "string" &&
        taskName.length > 0 &&
        !taskName.toLowerCase().includes("text generation")
      ) {
        return [];
      }
    }
    const raw =
      provider === "cloudflare_workers_ai"
        ? (model.name ?? model.id)
        : (model.id ?? model.name);
    if (typeof raw !== "string") return [];
    let id =
      provider === "google_generative_ai" ? raw.replace(/^models\//, "") : raw;
    if (provider === "google_vertex_ai") {
      const marker = "/models/";
      const idx = id.indexOf(marker);
      const stripped = idx >= 0 ? id.slice(idx + marker.length) : id;
      id = stripped.startsWith("google/") ? stripped : `google/${stripped}`;
    }
    if (!id.trim() || id.length > 200 || /[\r\n]/.test(id)) return [];
    const capabilities = record(model.capabilities);
    if (
      capabilities.completion_chat === false ||
      (capabilities.chat_completion === false &&
        capabilities.completion === false)
    )
      return [];
    const methods = model.supportedGenerationMethods;
    if (Array.isArray(methods) && !methods.includes("generateContent"))
      return [];
    const outputs = record(model.architecture).output_modalities;
    if (Array.isArray(outputs) && !outputs.includes("text")) return [];
    const inputs = record(model.architecture).input_modalities;
    if (Array.isArray(inputs) && !inputs.includes("text")) return [];
    if (Array.isArray(model.endpoints) && !model.endpoints.includes("chat"))
      return [];
    if (
      typeof model.type === "string" &&
      ["embedding", "embed", "rerank", "image", "audio"].includes(model.type)
    )
      return [];
    if (
      /(embed|tts|whisper|dall-e|audio|image|realtime|transcribe|moderation|rerank)/i.test(
        id,
      )
    )
      return [];
    return [id];
  });
  return [...new Set(models)].sort((a, b) =>
    a.localeCompare(b, undefined, { numeric: true }),
  );
}
