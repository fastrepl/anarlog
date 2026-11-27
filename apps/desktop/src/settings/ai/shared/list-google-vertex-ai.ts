import type {
  InputModality,
  ListModelsResult,
  ModelMetadata,
} from "./list-common";
import { processGenericModels } from "./list-openai";

import { providerFetch } from "~/ai/provider-fetch";

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
        item && typeof item === "object" && typeof (item as { id?: unknown }).id === "string"
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

  try {
    const res = await providerFetch(`${normalizedBase}/models`, {
      method: "GET",
      headers,
    });
    if (res.ok) {
      const parsed = parseVertexModelsPayload(await res.json());
      if (parsed) {
        return parsed;
      }
    }

    const parsedUrl = new URL(normalizedBase);
    if (parsedUrl.hostname.endsWith("aiplatform.googleapis.com")) {
      const pubRes = await providerFetch(
        `${parsedUrl.origin}/v1beta1/publishers/google/models`,
        { method: "GET", headers },
      );
      if (pubRes.ok) {
        const parsed = parseVertexModelsPayload(await pubRes.json());
        if (parsed) {
          return parsed;
        }
      }
    }
  } catch {
    return createStaticGoogleVertexAIModelResult();
  }

  return createStaticGoogleVertexAIModelResult();
}
