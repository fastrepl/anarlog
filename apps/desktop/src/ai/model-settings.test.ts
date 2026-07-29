import type { LanguageModel } from "ai";
import { describe, expect, it } from "vitest";

import { deterministicGenerationSettings } from "./model-settings";

function model(provider: string, modelId: string): LanguageModel {
  return {
    specificationVersion: "v3",
    provider,
    modelId,
    supportedUrls: {},
    doGenerate: async () => {
      throw new Error("not implemented");
    },
    doStream: async () => {
      throw new Error("not implemented");
    },
  };
}

describe("deterministicGenerationSettings", () => {
  it("omits temperature for Anthropic Claude 4.8 models", () => {
    expect(
      deterministicGenerationSettings(model("anthropic", "claude-opus-4-8")),
    ).toEqual({});
  });

  it("omits temperature for hosted Anthropic Claude 4.8 models", () => {
    expect(
      deterministicGenerationSettings(
        model("openrouter", "anthropic/claude-opus-4-8"),
      ),
    ).toEqual({});
    expect(
      deterministicGenerationSettings(
        model("anarlog", "anthropic/claude-opus-4-8"),
      ),
    ).toEqual({});
  });

  it("omits temperature for dotted Claude 4.8 model ids", () => {
    expect(
      deterministicGenerationSettings(model("anthropic", "claude-opus-4.8")),
    ).toEqual({});
  });

  it("omits temperature for GPT-5.6 Terra models", () => {
    expect(
      deterministicGenerationSettings(
        model("openai.responses", "gpt-5.6-terra"),
      ),
    ).toEqual({});
    expect(
      deterministicGenerationSettings(
        model("anarlog", "openai/gpt-5.6-terra-2026-07-28"),
      ),
    ).toEqual({});
  });

  it("keeps deterministic temperature for other models", () => {
    expect(
      deterministicGenerationSettings(model("anthropic", "claude-opus-4-5")),
    ).toEqual({ temperature: 0 });
  });
});
