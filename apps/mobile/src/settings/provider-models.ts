import { fetch } from "expo/fetch";

import { readProviderKey, readProviderSetup } from "./providers";
import {
  validateProviderConnection,
  type ProviderConfig,
} from "./providers-model";

export async function discoverProviderModels(
  account: string | null,
  config: ProviderConfig,
  signal: AbortSignal,
): Promise<string[]> {
  signal.throwIfAborted();
  const connection = validateProviderConnection("llm", config);
  const saved = await readProviderSetup(account, "llm", config.provider);
  if (saved.baseUrl !== connection.baseUrl)
    throw new Error("Provider connection changed. Reload models.");
  const apiKey = await readProviderKey(account, "llm", config.provider);
  if (!apiKey) throw new Error("Add an API key to load models.");
  signal.throwIfAborted();
  const controller = new AbortController();
  const abort = () => controller.abort();
  signal.addEventListener("abort", abort, { once: true });
  const timer = setTimeout(abort, 8_000);
  try {
    let url = `${connection.baseUrl}/models`;
    const headers: Record<string, string> = {};
    switch (config.provider) {
      case "anthropic":
        headers["x-api-key"] = apiKey;
        headers["anthropic-version"] = "2023-06-01";
        break;
      case "google_generative_ai":
        headers["x-goog-api-key"] = apiKey;
        break;
      case "azure_openai": {
        const base = connection.baseUrl.replace(/\/openai(?:\/v1)?$/, "");
        url = `${base}/openai/models?api-version=2024-10-21`;
        headers["api-key"] = apiKey;
        break;
      }
      case "azure_ai":
        headers["api-key"] = apiKey;
        break;
      default:
        headers.Authorization = `Bearer ${apiKey}`;
    }
    const response = await fetch(url, {
      headers,
      signal: controller.signal,
      redirect: "error",
    });
    if (!response.ok) {
      await response.body?.cancel();
      throw new Error("Model request failed.");
    }
    return parseProviderModels(
      config.provider,
      JSON.parse(await readModels(response)),
    );
  } catch {
    throw new Error("Couldn’t load models. Retry or enter a model ID.");
  } finally {
    clearTimeout(timer);
    signal.removeEventListener("abort", abort);
  }
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

export function parseProviderModels(
  provider: string,
  response: unknown,
): string[] {
  const body = record(response);
  const data = body.data ?? body.models;
  if (!Array.isArray(data)) throw new Error("Invalid model list.");
  const models = data.flatMap((entry: unknown) => {
    const model = record(entry);
    const raw = model.id ?? model.name;
    if (typeof raw !== "string") return [];
    const id =
      provider === "google_generative_ai" ? raw.replace(/^models\//, "") : raw;
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
