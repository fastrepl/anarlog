import { describe, expect, it } from "vitest";

import {
  normalizeReasoningEffort,
  reasoningProviderOptions,
} from "./reasoning-effort";

describe("normalizeReasoningEffort", () => {
  it("normalizes unknown or missing levels to default", () => {
    expect(normalizeReasoningEffort("high")).toBe("high");
    expect(normalizeReasoningEffort(undefined)).toBe("default");
    expect(normalizeReasoningEffort("")).toBe("default");
    expect(normalizeReasoningEffort("xhigh")).toBe("default");
  });
});

describe("reasoningProviderOptions", () => {
  it.each([
    ["openai", "gpt-5", "default", null],
    ["anarlog", "Auto", "high", null],
    ["apple_foundation", "default", "high", null],
    ["openai", "gpt-5", "low", { openai: { reasoningEffort: "low" } }],
    ["azure_openai", "gpt-5", "high", { azure: { reasoningEffort: "high" } }],
    [
      "anthropic",
      "claude-opus-5",
      "medium",
      {
        anthropic: { thinking: { type: "adaptive" }, effort: "medium" },
      },
    ],
    [
      "openrouter",
      "openai/gpt-5",
      "high",
      { openrouter: { reasoning: { effort: "high" } } },
    ],
    [
      "google_generative_ai",
      "gemini-3.8-flash",
      "high",
      { google: { thinkingConfig: { thinkingLevel: "high" } } },
    ],
    [
      "google_generative_ai",
      "gemini-2.5-flash",
      "medium",
      { google: { thinkingConfig: { thinkingBudget: 8192 } } },
    ],
    ["google_generative_ai", "gemini-2.0-flash", "high", null],
    ["ollama", "gpt-oss:20b", "low", { ollama: { reasoningEffort: "low" } }],
  ] as const)(
    "maps %s/%s with %s reasoning effort",
    (providerId, modelId, effort, expected) => {
      expect(reasoningProviderOptions(providerId, modelId, effort)).toEqual(
        expected,
      );
    },
  );
});
