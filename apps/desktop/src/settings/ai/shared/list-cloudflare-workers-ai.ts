import type { ListModelsResult, ModelMetadata } from "./list-common";
import { DEFAULT_RESULT } from "./list-common";

import { providerFetch } from "~/ai/provider-fetch";

export const CLOUDFLARE_WORKERS_AI_MODELS = [
  "@cf/moonshotai/kimi-k2.7-code",
  "@cf/zai-org/glm-5.2",
  "@cf/moonshotai/kimi-k2.6",
  "@cf/zai-org/glm-4.7-flash",
  "@cf/openai/gpt-oss-120b",
  "@cf/meta/llama-4-scout-17b-16e-instruct",
  "@cf/google/gemma-4-26b-a4b-it",
  "@cf/nvidia/nemotron-3-120b-a12b",
  "@cf/openai/gpt-oss-20b",
  "@cf/qwen/qwen3-30b-a3b-fp8",
  "@cf/mistralai/mistral-small-3.1-24b-instruct",
  "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
] as const;

const VISION_MODELS = new Set<string>([
  "@cf/moonshotai/kimi-k2.7-code",
  "@cf/moonshotai/kimi-k2.6",
  "@cf/meta/llama-4-scout-17b-16e-instruct",
  "@cf/google/gemma-4-26b-a4b-it",
]);

function isCloudflareWorkersAIModel(
  model: string,
): model is (typeof CLOUDFLARE_WORKERS_AI_MODELS)[number] {
  return (CLOUDFLARE_WORKERS_AI_MODELS as readonly string[]).includes(model);
}

export function getCloudflareWorkersAIModelMetadata(
  model: string | undefined,
): ModelMetadata | undefined {
  if (!model || !isCloudflareWorkersAIModel(model)) {
    return undefined;
  }

  return {
    input_modalities: VISION_MODELS.has(model) ? ["text", "image"] : ["text"],
  };
}

export function createStaticCloudflareWorkersAIModelResult(): ListModelsResult {
  const metadata: Record<string, ModelMetadata> = {};

  for (const model of CLOUDFLARE_WORKERS_AI_MODELS) {
    metadata[model] = getCloudflareWorkersAIModelMetadata(model)!;
  }

  return {
    models: [...CLOUDFLARE_WORKERS_AI_MODELS],
    ignored: [],
    metadata,
  };
}

type WorkersAIRawModel = {
  id?: unknown;
  name?: unknown;
  task?: { name?: unknown };
  properties?: Array<{ property_id?: unknown; value?: unknown }>;
};

const NON_CHAT_CLOUDFLARE_MODEL_PATTERNS = [
  "embed",
  "bge-",
  "rerank",
  "whisper",
  "tts",
  "flux",
  "stable-diffusion",
];

function buildCloudflareModelsResult(ids: string[]): ListModelsResult | null {
  const uniqueIds: string[] = [];
  const seen = new Set<string>();
  const metadata: Record<string, ModelMetadata> = {};

  for (const rawId of ids) {
    const id = rawId.trim();
    if (!id || seen.has(id)) {
      continue;
    }
    const lower = id.toLowerCase();
    if (
      NON_CHAT_CLOUDFLARE_MODEL_PATTERNS.some((pattern) =>
        lower.includes(pattern),
      )
    ) {
      continue;
    }
    seen.add(id);
    uniqueIds.push(id);
    metadata[id] = getCloudflareWorkersAIModelMetadata(id) ?? {
      input_modalities: ["text"],
    };
  }

  if (uniqueIds.length === 0) {
    return null;
  }

  return {
    models: uniqueIds,
    ignored: [],
    metadata,
  };
}

function parseCloudflareModelsResponse(payload: unknown): ListModelsResult | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const obj = payload as { data?: unknown; result?: unknown };
  if (Array.isArray(obj.data)) {
    const ids = obj.data
      .filter(
        (item): item is { id: string } =>
          Boolean(item) &&
          typeof item === "object" &&
          typeof (item as { id?: unknown }).id === "string",
      )
      .map((item) => item.id);
    const parsed = buildCloudflareModelsResult(ids);
    if (parsed) {
      return parsed;
    }
  }

  if (Array.isArray(obj.result)) {
    const rawItems = obj.result as WorkersAIRawModel[];
    const ids = rawItems
      .filter((item) => {
        const taskName =
          typeof item.task?.name === "string"
            ? item.task.name.toLowerCase()
            : "";
        return !taskName || taskName.includes("text generation");
      })
      .map((item) =>
        typeof item.name === "string"
          ? item.name
          : typeof item.id === "string"
            ? item.id
            : "",
      );
    const parsed = buildCloudflareModelsResult(ids);
    if (parsed) {
      return parsed;
    }
  }

  return null;
}

export async function listCloudflareWorkersAIModels(
  baseUrl: string,
  apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  const trimmedKey = apiKey.trim();
  if (!trimmedKey) {
    return createStaticCloudflareWorkersAIModelResult();
  }

  const normalized = baseUrl.trim().replace(/\/+$/, "");
  const headers = { Authorization: `Bearer ${trimmedKey}` };

  try {
    const res = await providerFetch(`${normalized}/models`, {
      method: "GET",
      headers,
    });
    if (res.ok) {
      const parsed = parseCloudflareModelsResponse(await res.json());
      if (parsed) {
        return parsed;
      }
    }

    if (/\/client\/v4\/accounts\/[^/]+\/ai(?:\/v1)?$/i.test(normalized)) {
      const searchBase = normalized.replace(/\/v1$/i, "");
      const searchRes = await providerFetch(
        `${searchBase}/models/search?task=Text%20Generation`,
        {
          method: "GET",
          headers,
        },
      );
      if (searchRes.ok) {
        const parsed = parseCloudflareModelsResponse(await searchRes.json());
        if (parsed) {
          return parsed;
        }
      }
    }
  } catch {
    return createStaticCloudflareWorkersAIModelResult();
  }

  return createStaticCloudflareWorkersAIModelResult();
}
