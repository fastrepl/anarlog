import type {
  InputModality,
  ListModelsResult,
  ModelMetadata,
} from "./list-common";

// Fallback list when GET /llm/models is unreachable; kept aligned with crates/llm-proxy/src/model.rs.
export const ANARLOG_PRO_LLM_MODELS = [
  { id: "Auto", label: "Pro (Cloud)", inputModalities: ["text", "image"] },
  {
    id: "~anthropic/claude-sonnet-latest",
    label: "Claude Sonnet",
    inputModalities: ["text", "image"],
  },
  {
    id: "~anthropic/claude-opus-latest",
    label: "Claude Opus",
    inputModalities: ["text", "image"],
  },
  {
    id: "~anthropic/claude-haiku-latest",
    label: "Claude Haiku",
    inputModalities: ["text", "image"],
  },
  {
    id: "~openai/gpt-sol-latest",
    label: "GPT Sol",
    inputModalities: ["text", "image"],
  },
  {
    id: "~openai/gpt-astra-latest",
    label: "GPT Astra",
    inputModalities: ["text", "image"],
  },
  {
    id: "~openai/gpt-terra-latest",
    label: "GPT Terra",
    inputModalities: ["text", "image"],
  },
  {
    id: "~openai/gpt-luna-latest",
    label: "GPT Luna",
    inputModalities: ["text", "image"],
  },
  {
    id: "~google/gemini-pro-latest",
    label: "Gemini Pro",
    inputModalities: ["text", "image"],
  },
  {
    id: "~google/gemini-flash-latest",
    label: "Gemini Flash",
    inputModalities: ["text", "image"],
  },
  {
    id: "~x-ai/grok-latest",
    label: "Grok",
    inputModalities: ["text", "image"],
  },
  {
    id: "~moonshotai/kimi-latest",
    label: "Kimi",
    inputModalities: ["text", "image"],
  },
  {
    id: "~deepseek/deepseek-pro-latest",
    label: "DeepSeek Pro",
    inputModalities: ["text"],
  },
  {
    id: "~deepseek/deepseek-flash-latest",
    label: "DeepSeek Flash",
    inputModalities: ["text", "image"],
  },
  { id: "~z-ai/glm-latest", label: "GLM", inputModalities: ["text"] },
  {
    id: "~z-ai/glm-flash-latest",
    label: "GLM Flash",
    inputModalities: ["text", "image"],
  },
] as const;

const KNOWN_MODALITIES = new Map<string, readonly InputModality[]>(
  ANARLOG_PRO_LLM_MODELS.map(({ id, inputModalities }) => [id, inputModalities]),
);

const KNOWN_LABELS = new Map<string, string>(
  ANARLOG_PRO_LLM_MODELS.map(({ id, label }) => [id, label]),
);

export function anarlogProLlmModelLabel(model: string): string | undefined {
  return KNOWN_LABELS.get(model);
}

export function createStaticAnarlogProModelResult(): ListModelsResult {
  return {
    models: ANARLOG_PRO_LLM_MODELS.map(({ id }) => id),
    ignored: [],
    metadata: Object.fromEntries(
      ANARLOG_PRO_LLM_MODELS.map(({ id, inputModalities }) => [
        id,
        { input_modalities: [...inputModalities] },
      ]),
    ),
  };
}

type LlmProxyModelsPayload = {
  data?: Array<{
    id?: unknown;
    label?: unknown;
    input_modalities?: unknown;
  }>;
};

function parseModalities(value: unknown): InputModality[] | undefined {
  if (!Array.isArray(value)) {
    return undefined;
  }
  const filtered = value.filter(
    (entry): entry is InputModality => entry === "text" || entry === "image",
  );
  return filtered.length > 0 ? filtered : undefined;
}

export async function listAnarlogProModels(
  baseUrl: string,
  fetcher: typeof fetch,
): Promise<ListModelsResult> {
  const normalizedBase = baseUrl.trim().replace(/\/+$/, "");
  if (!normalizedBase) {
    return createStaticAnarlogProModelResult();
  }

  try {
    const response = await fetcher(`${normalizedBase}/models`, {
      method: "GET",
    });
    if (!response.ok) {
      return createStaticAnarlogProModelResult();
    }

    const payload = (await response.json()) as LlmProxyModelsPayload;
    const models: string[] = [];
    const metadata: Record<string, ModelMetadata> = {};
    const seen = new Set<string>();

    for (const item of payload.data ?? []) {
      const id = typeof item.id === "string" ? item.id.trim() : "";
      if (!id || seen.has(id)) {
        continue;
      }
      seen.add(id);
      models.push(id);
      const label = typeof item.label === "string" ? item.label.trim() : "";
      if (label) {
        KNOWN_LABELS.set(id, label);
      }
      const modalities =
        parseModalities(item.input_modalities) ??
        KNOWN_MODALITIES.get(id) ?? ["text"];
      metadata[id] = { input_modalities: [...modalities] };
    }

    if (models.length === 0) {
      return createStaticAnarlogProModelResult();
    }

    return { models, ignored: [], metadata };
  } catch {
    return createStaticAnarlogProModelResult();
  }
}
