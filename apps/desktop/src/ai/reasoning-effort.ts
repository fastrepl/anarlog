import type { defaultSettingsMiddleware } from "ai";

export const REASONING_EFFORTS = ["default", "low", "medium", "high"] as const;
export type ReasoningEffort = (typeof REASONING_EFFORTS)[number];

export const normalizeReasoningEffort = (value: unknown): ReasoningEffort =>
  REASONING_EFFORTS.includes(value as ReasoningEffort)
    ? (value as ReasoningEffort)
    : "default";

// Anarlog Pro picks its own model settings, and Apple Foundation has no
// reasoning knob to turn.
export const supportsReasoningEffort = (providerId: string): boolean =>
  providerId !== "anarlog" && providerId !== "apple_foundation";

type ProviderOptions = NonNullable<
  Parameters<typeof defaultSettingsMiddleware>[0]["settings"]["providerOptions"]
>;

const GEMINI_THINKING_BUDGET = { low: 1024, medium: 8192, high: 24576 };

// Gemini 2.5 only understands a token budget, thinkingLevel is 3.x+, and
// older generations reject thinkingConfig outright, so unknown ids send nothing.
const geminiThinkingConfig = (
  modelId: string,
  effort: Exclude<ReasoningEffort, "default">,
) => {
  const version = /gemini-(\d+)(?:\.(\d+))?/.exec(modelId);
  if (!version) {
    return null;
  }
  const major = Number(version[1]);
  const minor = Number(version[2] ?? 0);
  if (major >= 3) {
    return { thinkingLevel: effort };
  }
  if (major === 2 && minor === 5) {
    return { thinkingBudget: GEMINI_THINKING_BUDGET[effort] };
  }
  return null;
};

export const reasoningProviderOptions = (
  providerId: string,
  modelId: string,
  effort: ReasoningEffort,
): ProviderOptions | null => {
  if (effort === "default" || !supportsReasoningEffort(providerId)) {
    return null;
  }

  switch (providerId) {
    case "openai":
    case "chatgpt":
      return { openai: { reasoningEffort: effort } };
    case "azure_openai":
      return { azure: { reasoningEffort: effort } };
    case "anthropic":
      return { anthropic: { thinking: { type: "adaptive" }, effort } };
    case "openrouter":
      return { openrouter: { reasoning: { effort } } };
    case "google_generative_ai": {
      const thinkingConfig = geminiThinkingConfig(modelId, effort);
      return thinkingConfig ? { google: { thinkingConfig } } : null;
    }
    default:
      // Every other provider goes through @ai-sdk/openai-compatible, which
      // reads options under the provider name and sends `reasoning_effort`.
      return { [providerId]: { reasoningEffort: effort } };
  }
};
