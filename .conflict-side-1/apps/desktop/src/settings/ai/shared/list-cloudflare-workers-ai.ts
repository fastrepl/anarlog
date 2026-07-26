import type { ListModelsResult, ModelMetadata } from "./list-common";
import { DEFAULT_RESULT } from "./list-common";

export const CLOUDFLARE_WORKERS_AI_MODELS = [
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

export async function listCloudflareWorkersAIModels(
  baseUrl: string,
  _apiKey: string,
): Promise<ListModelsResult> {
  if (!baseUrl) {
    return DEFAULT_RESULT;
  }

  return createStaticCloudflareWorkersAIModelResult();
}
