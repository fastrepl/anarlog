import { describe, expect, it } from "vitest";

import {
  normalizeReasoningEffort,
  reasoningProviderOptions,
  supportsReasoningEffort,
} from "./reasoning-effort";

describe("normalizeReasoningEffort", () => {
  it("keeps known levels", () => {
    expect(normalizeReasoningEffort("high")).toBe("high");
  });

  it("falls back to default for unknown or missing values", () => {
    expect(normalizeReasoningEffort(undefined)).toBe("default");
    expect(normalizeReasoningEffort("")).toBe("default");
    expect(normalizeReasoningEffort("xhigh")).toBe("default");
  });
});

describe("supportsReasoningEffort", () => {
  it("leaves Anarlog Pro and Apple Foundation out", () => {
    expect(supportsReasoningEffort("anarlog")).toBe(false);
    expect(supportsReasoningEffort("apple_foundation")).toBe(false);
    expect(supportsReasoningEffort("openai")).toBe(true);
  });
});

describe("reasoningProviderOptions", () => {
  it("sends nothing for the default level", () => {
    expect(reasoningProviderOptions("openai", "gpt-5", "default")).toBeNull();
  });

  it("sends nothing for providers without a knob", () => {
    expect(reasoningProviderOptions("anarlog", "Auto", "high")).toBeNull();
    expect(
      reasoningProviderOptions("apple_foundation", "default", "high"),
    ).toBeNull();
  });

  it("maps OpenAI-family providers to reasoningEffort", () => {
    expect(reasoningProviderOptions("openai", "gpt-5", "low")).toEqual({
      openai: { reasoningEffort: "low" },
    });
    expect(reasoningProviderOptions("chatgpt", "gpt-5", "medium")).toEqual({
      openai: { reasoningEffort: "medium" },
    });
    expect(reasoningProviderOptions("azure_openai", "gpt-5", "high")).toEqual({
      azure: { reasoningEffort: "high" },
    });
  });

  it("maps Anthropic to adaptive thinking with effort", () => {
    expect(
      reasoningProviderOptions("anthropic", "claude-opus-5", "medium"),
    ).toEqual({
      anthropic: { thinking: { type: "adaptive" }, effort: "medium" },
    });
  });

  it("maps OpenRouter to reasoning.effort", () => {
    expect(
      reasoningProviderOptions("openrouter", "openai/gpt-5", "high"),
    ).toEqual({ openrouter: { reasoning: { effort: "high" } } });
  });

  it("maps Gemini 3+ to thinkingLevel and Gemini 2.5 to a budget", () => {
    expect(
      reasoningProviderOptions("google_generative_ai", "gemini-3-pro", "low"),
    ).toEqual({ google: { thinkingConfig: { thinkingLevel: "low" } } });
    expect(
      reasoningProviderOptions(
        "google_generative_ai",
        "gemini-3.8-flash",
        "high",
      ),
    ).toEqual({ google: { thinkingConfig: { thinkingLevel: "high" } } });
    expect(
      reasoningProviderOptions(
        "google_generative_ai",
        "gemini-2.5-flash",
        "medium",
      ),
    ).toEqual({ google: { thinkingConfig: { thinkingBudget: 8192 } } });
  });

  it("sends nothing for Gemini models without a thinking config", () => {
    for (const modelId of [
      "gemini-2.0-flash",
      "gemini-1.5-pro",
      "gemini-flash-latest",
    ]) {
      expect(
        reasoningProviderOptions("google_generative_ai", modelId, "high"),
      ).toBeNull();
    }
  });

  it("keys OpenAI-compatible providers by their provider id", () => {
    expect(reasoningProviderOptions("ollama", "gpt-oss:20b", "low")).toEqual({
      ollama: { reasoningEffort: "low" },
    });
    expect(reasoningProviderOptions("github_copilot", "gpt-5", "high")).toEqual(
      { github_copilot: { reasoningEffort: "high" } },
    );
    expect(reasoningProviderOptions("custom", "my-model", "medium")).toEqual({
      custom: { reasoningEffort: "medium" },
    });
  });
});
