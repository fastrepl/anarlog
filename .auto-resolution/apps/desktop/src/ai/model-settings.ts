import type { LanguageModel } from "ai";

export function deterministicGenerationSettings(model: LanguageModel): {
  temperature?: number;
} {
  if (usesDeprecatedTemperature(model)) {
    return {};
  }

  return { temperature: 0 };
}

function usesDeprecatedTemperature(model: LanguageModel): boolean {
  if (typeof model === "string") {
    return false;
  }

  const provider = "provider" in model ? model.provider : "";
  const modelId = "modelId" in model ? model.modelId : "";
  const normalizedModelId = modelId.toLowerCase().replace(/\./g, "-");
  const modelName = normalizedModelId.includes("/")
    ? normalizedModelId.split("/").pop()!
    : normalizedModelId;

  return (
    (provider.startsWith("anthropic") ||
      normalizedModelId.startsWith("anthropic/")) &&
    /^claude-(?:opus|sonnet|haiku)-4-8(?:$|-)/.test(modelName)
  );
}
