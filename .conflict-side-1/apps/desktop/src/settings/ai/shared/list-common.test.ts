import { describe, expect, test } from "vitest";

import { isOldModel, sortModelsByRecency } from "./list-common";

describe("isOldModel", () => {
  test("filters older OpenAI chat families while keeping current models", () => {
    expect(isOldModel("gpt-4o")).toBe(true);
    expect(isOldModel("gpt-4.1")).toBe(true);
    expect(isOldModel("gpt-5")).toBe(true);
    expect(isOldModel("gpt-5.1-chat-latest")).toBe(true);

    expect(isOldModel("gpt-5.4-mini")).toBe(false);
    expect(isOldModel("gpt-5.5")).toBe(false);
    expect(isOldModel("chat-latest")).toBe(false);
  });

  test("filters older Claude families without hiding current pinned IDs", () => {
    expect(isOldModel("claude-3-7-sonnet")).toBe(true);
    expect(isOldModel("anthropic/claude-opus-4.7")).toBe(true);
    expect(isOldModel("claude-sonnet-4-5")).toBe(true);
    expect(isOldModel("claude-sonnet-4-6")).toBe(true);

    expect(isOldModel("claude-fable-5")).toBe(false);
    expect(isOldModel("claude-opus-4-8")).toBe(false);
    expect(isOldModel("claude-sonnet-5")).toBe(false);
    expect(isOldModel("claude-sonnet-latest")).toBe(false);
    expect(isOldModel("claude-haiku-4-5-20251001")).toBe(false);
  });

  test("filters older Gemini and Mistral families", () => {
    expect(isOldModel("gemini-2.5-pro")).toBe(true);
    expect(isOldModel("mistral-small-2506")).toBe(true);
    expect(isOldModel("mistral-medium-3.1")).toBe(true);

    expect(isOldModel("gemini-3.5-flash")).toBe(false);
    expect(isOldModel("gemini-3.1-pro-preview")).toBe(false);
    expect(isOldModel("mistral-medium-3-5")).toBe(false);
    expect(isOldModel("mistral-large-2512")).toBe(false);
  });
});

describe("sortModelsByRecency", () => {
  test("prioritizes current hosted models across provider-prefixed IDs", () => {
    expect(
      sortModelsByRecency([
        "openai/gpt-5.4-mini",
        "anthropic/claude-sonnet-4.6",
        "anthropic/claude-sonnet-5",
        "openai/gpt-5.5",
        "google/gemini-3.5-flash",
        "openai/chat-latest",
      ]),
    ).toEqual([
      "openai/gpt-5.5",
      "openai/chat-latest",
      "anthropic/claude-sonnet-5",
      "openai/gpt-5.4-mini",
      "anthropic/claude-sonnet-4.6",
      "google/gemini-3.5-flash",
    ]);
  });
});
