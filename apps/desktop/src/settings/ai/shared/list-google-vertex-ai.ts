import { Duration } from "effect";

import {
  type InputModality,
  type ListModelsResult,
  type ModelMetadata,
  readResponseTextWithLimit,
  REQUEST_TIMEOUT,
} from "./list-common";
import { processGenericModels } from "./list-openai";

import { providerFetch } from "~/ai/provider-fetch";

const MAX_VERTEX_RESPONSE_BYTES = 8 * 1024 * 1024;
const VERTEX_DISCOVERY_TIMEOUT_MS = Duration.toMillis(REQUEST_TIMEOUT);

export const GOOGLE_VERTEX_AI_MODELS = [
  "google/gemini-3.8-flash",
  "google/gemini-3.7-flash",
  "google/gemini-3.6-flash",
  "google/gemini-3.5-flash-lite",
  "google/gemini-3.1-pro-preview",
  "google/gemini-3.5-flash",
  "google/gemini-3.1-flash-lite",
] as const;

export function createStaticGoogleVertexAIModelResult(): ListModelsResult {
  return {
    models: [...GOOGLE_VERTEX_AI_MODELS],
    ignored: [],
    metadata: Object.fromEntries(
      GOOGLE_VERTEX_AI_MODELS.map((model) => [
        model,
        { input_modalities: ["text", "image"] as InputModality[] },
      ]),
    ),
  };
}

function normalizeVertexModelId(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed.includes("publishers/google/models/")) {
    const suffix = trimmed.split("publishers/google/models/").pop() ?? trimmed;
    return `google/${suffix.replace(/^models\//, "")}`;
  }
  return trimmed.replace(/^models\//, "");
}

function parseVertexModelsPayload(payload: unknown): ListModelsResult | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const obj = payload as {
    data?: unknown;
    models?: unknown;
    publisherModels?: unknown;
  };
  const candidates: Array<{ id: string }> = [];

  if (Array.isArray(obj.data)) {
    for (const item of obj.data) {
      const id =
        item &&
        typeof item === "object" &&
        typeof (item as { id?: unknown }).id === "string"
          ? normalizeVertexModelId((item as { id: string }).id)
          : "";
      if (id) {
        candidates.push({ id });
      }
    }
  } else if (Array.isArray(obj.publisherModels)) {
    for (const item of obj.publisherModels) {
      const name =
        item &&
        typeof item === "object" &&
        typeof (item as { name?: unknown }).name === "string"
          ? normalizeVertexModelId((item as { name: string }).name)
          : "";
      if (name) {
        candidates.push({ id: name });
      }
    }
  } else if (Array.isArray(obj.models)) {
    for (const item of obj.models) {
      if (!item || typeof item !== "object") {
        continue;
      }
      const raw = item as { id?: unknown; name?: unknown };
      const rawId =
        typeof raw.id === "string"
          ? raw.id
          : typeof raw.name === "string"
            ? raw.name
            : "";
      const normalized = rawId ? normalizeVertexModelId(rawId) : "";
      if (normalized) {
        candidates.push({ id: normalized });
      }
    }
  }

  if (candidates.length === 0) {
    return null;
  }

  const parsed = processGenericModels(candidates);
  if (parsed.models.length === 0) {
    return null;
  }

  const metadata: Record<string, ModelMetadata> = {};
  for (const id of parsed.models) {
    metadata[id] = {
      input_modalities: id.includes("gemini") ? ["text", "image"] : ["text"],
    };
  }

  return { ...parsed, metadata };
}

async function fetchVertexJson(
  url: string,
  headers: Record<string, string>,
  signal: AbortSignal,
): Promise<unknown | null> {
  const res = await providerFetch(url, { method: "GET", headers, signal });
  if (!res.ok) {
    await res.body?.cancel().catch(() => {});
    return null;
  }

  let abortListener: (() => void) | undefined;
  const abortPromise = new Promise<never>((_, reject) => {
    const onAbort = () => {
      void res.body?.cancel().catch(() => {});
      reject(signal.reason ?? new Error("Vertex model discovery timed out"));
    };
    if (signal.aborted) {
      onAbort();
      return;
    }
    abortListener = onAbort;
    signal.addEventListener("abort", onAbort, { once: true });
  });

  try {
    const text = await Promise.race([
      readResponseTextWithLimit(res, MAX_VERTEX_RESPONSE_BYTES),
      abortPromise,
    ]);
    return JSON.parse(text) as unknown;
  } finally {
    if (abortListener) {
      signal.removeEventListener("abort", abortListener);
    }
  }
}

export async function listGoogleVertexAIModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  const normalizedBase = baseUrl.trim().replace(/\/+$/, "");
  const trimmedKey = apiKey.trim();
  if (!normalizedBase || !trimmedKey) {
    return createStaticGoogleVertexAIModelResult();
  }

  const headers = { Authorization: `Bearer ${trimmedKey}` };
  const controller = new AbortController();
  const timeoutId = setTimeout(
    () => controller.abort(new Error("Vertex model discovery timed out")),
    VERTEX_DISCOVERY_TIMEOUT_MS,
  );

  try {
    const primaryPayload = await fetchVertexJson(
      `${normalizedBase}/models`,
      headers,
      controller.signal,
    );
    const parsedPrimary = parseVertexModelsPayload(primaryPayload);
    if (parsedPrimary) {
      return parsedPrimary;
    }

    const parsedUrl = new URL(normalizedBase);
    if (parsedUrl.hostname.endsWith("aiplatform.googleapis.com")) {
      const publisherPayload = await fetchVertexJson(
        `${parsedUrl.origin}/v1beta1/publishers/google/models`,
        headers,
        controller.signal,
      );
      const parsedPublisher = parseVertexModelsPayload(publisherPayload);
      if (parsedPublisher) {
        return parsedPublisher;
      }
    }
  } catch {
    return createStaticGoogleVertexAIModelResult();
  } finally {
    clearTimeout(timeoutId);
  }

  return createStaticGoogleVertexAIModelResult();
}
