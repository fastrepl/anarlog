import type { LanguageModel } from "ai";

export function deterministicGenerationSettings(model: LanguageModel): {
  temperature?: number;
} {
  if (requiresDefaultTemperature(model)) {
    return {};
  }

  return { temperature: 0 };
}

function requiresDefaultTemperature(model: LanguageModel): boolean {
  if (typeof model === "string") {
    return false;
  }

  const provider = "provider" in model ? model.provider : "";
  const modelId = "modelId" in model ? model.modelId : "";
  const normalizedModelId = modelId.toLowerCase().replace(/\./g, "-");
  const modelName = normalizedModelId.includes("/")
    ? normalizedModelId.split("/").pop()!
    : normalizedModelId;

  const isClaude48 =
    (provider.startsWith("anthropic") ||
      normalizedModelId.startsWith("anthropic/")) &&
    /^claude-(?:opus|sonnet|haiku)-4-8(?:$|-)/.test(modelName);
  const isGpt56Terra = /^gpt-5-6-terra(?:$|-)/.test(modelName);

  return isClaude48 || isGpt56Terra;
}
