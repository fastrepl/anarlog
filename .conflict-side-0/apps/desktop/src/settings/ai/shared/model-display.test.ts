import { describe, expect, test } from "vitest";

import { displayLlmModelId } from "./model-display";

describe("displayLlmModelId", () => {
  test("uses stable names for latest aliases", () => {
    expect(
      displayLlmModelId("openrouter", "~anthropic/claude-sonnet-latest"),
    ).toBe("Claude Sonnet 5");
    expect(displayLlmModelId("openai", "chat-latest")).toBe("Chat Latest");
  });

  test("removes provider prefixes and release dates", () => {
    expect(
      displayLlmModelId("openrouter", "anthropic/claude-haiku-4-5-20251001"),
    ).toBe("Claude Haiku 4.5");
    expect(
      displayLlmModelId("openrouter", "mistralai/mistral-large-2512"),
    ).toBe("Mistral Large");
  });

  test("formats common model families without changing stored ids", () => {
    expect(displayLlmModelId("openai", "gpt-5.5")).toBe("GPT 5.5");
    expect(displayLlmModelId("google_generative_ai", "gemini-3.5-flash")).toBe(
      "Gemini 3.5 Flash",
    );
  });

  test("keeps managed cloud label product-facing", () => {
    expect(displayLlmModelId("hyprnote", "Auto")).toBe("Pro (Cloud)");
  });
});
