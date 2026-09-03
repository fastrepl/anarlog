import { describe, expect, test } from "vitest";

import {
  isDateSnapshot,
  isNonStreamingModel,
  isOldModel,
  readResponseTextWithLimit,
  removeNonStreamingModels,
  sortModelsByRecency,
} from "./list-common";

describe("readResponseTextWithLimit", () => {
  test("reads a response within the byte limit", async () => {
    await expect(
      readResponseTextWithLimit(new Response("hello"), 5),
    ).resolves.toBe("hello");
  });

  test("cancels a response that exceeds the byte limit", async () => {
    await expect(
      readResponseTextWithLimit(new Response("toolong"), 6),
    ).rejects.toThrow("Response body exceeds 6 bytes");
  });
});

describe("isDateSnapshot", () => {
  test("keeps provider version and context suffixes", () => {
    expect(isDateSnapshot("llama-3.1-8b-instant-8192")).toBe(false);
    expect(isDateSnapshot("grok-4-0709")).toBe(false);
    expect(isDateSnapshot("command-a-plus-05-2026")).toBe(true);
  });
});

describe("isNonStreamingModel", () => {
  test("filters GPT Pro models without hiding streaming-capable models", () => {
    expect(isNonStreamingModel("gpt-5.4-pro")).toBe(true);
    expect(isNonStreamingModel("openai/gpt-5.5-pro")).toBe(true);
    expect(isNonStreamingModel("gpt-5.5-pro-2026-04-23")).toBe(true);

    expect(isNonStreamingModel("gpt-5.5")).toBe(false);
    expect(isNonStreamingModel("gemini-3.1-pro-preview")).toBe(false);
  });
});

describe("removeNonStreamingModels", () => {
  test("removes non-streaming models from provider API results", () => {
    expect(
      removeNonStreamingModels({
        models: ["openai/gpt-5.5-pro", "openai/gpt-5.5"],
        ignored: [
          { id: "gpt-5.4-pro", reasons: ["date_snapshot"] },
          { id: "gpt-5.4-mini", reasons: ["date_snapshot"] },
        ],
        metadata: {
          "openai/gpt-5.5-pro": { input_modalities: ["text", "image"] },
          "openai/gpt-5.5": { input_modalities: ["text", "image"] },
        },
      }),
    ).toEqual({
      models: ["openai/gpt-5.5"],
      ignored: [{ id: "gpt-5.4-mini", reasons: ["date_snapshot"] }],
      metadata: {
        "openai/gpt-5.5": { input_modalities: ["text", "image"] },
      },
    });
  });
});

describe("isOldModel", () => {
  test("filters older OpenAI chat families while keeping current models", () => {
    expect(isOldModel("gpt-4o")).toBe(true);
    expect(isOldModel("gpt-4.1")).toBe(true);
    expect(isOldModel("gpt-5")).toBe(true);
    expect(isOldModel("gpt-5.1-chat-latest")).toBe(true);

    expect(isOldModel("gpt-5.4-mini")).toBe(false);
    expect(isOldModel("gpt-5.5")).toBe(false);
    expect(isOldModel("gpt-5.6-sol")).toBe(false);
    expect(isOldModel("chat-latest")).toBe(false);
  });

  test("filters retired xAI, DeepSeek, Moonshot, Z.AI, and Cohere ids", () => {
    expect(isOldModel("grok-3")).toBe(true);
    expect(isOldModel("grok-4")).toBe(true);
    expect(isOldModel("grok-4-fast-reasoning")).toBe(true);
    expect(isOldModel("deepseek-chat")).toBe(true);
    expect(isOldModel("deepseek-reasoner")).toBe(true);
    expect(isOldModel("kimi-k2.5")).toBe(true);
    expect(isOldModel("moonshot-v1-128k")).toBe(true);
    expect(isOldModel("glm-4.7")).toBe(true);
    expect(isOldModel("glm-5.1")).toBe(true);
    expect(isOldModel("command-r-plus")).toBe(true);
    expect(isOldModel("llama-3.3-70b-versatile")).toBe(true);
    expect(isOldModel("qwen3-235b-a22b")).toBe(true);
    expect(isOldModel("qwen-max")).toBe(true);

    expect(isOldModel("grok-4.6")).toBe(false);
    expect(isOldModel("deepseek-v4-pro")).toBe(false);
    expect(isOldModel("kimi-k3")).toBe(false);
    expect(isOldModel("kimi-k2.7-code")).toBe(false);
    expect(isOldModel("glm-5.3")).toBe(false);
    expect(isOldModel("command-a-plus-05-2026")).toBe(false);
    expect(isOldModel("command-r7b-12-2024")).toBe(false);
    expect(isOldModel("qwen3.8-max")).toBe(false);
  });

  test("filters older Claude families without hiding current pinned IDs", () => {
    expect(isOldModel("claude-3-7-sonnet")).toBe(true);
    expect(isOldModel("anthropic/claude-opus-4.7")).toBe(true);
    expect(isOldModel("claude-sonnet-4-5")).toBe(true);
    expect(isOldModel("claude-sonnet-4-6")).toBe(true);

    expect(isOldModel("claude-fable-5")).toBe(false);
    expect(isOldModel("claude-fable-5-1")).toBe(false);
    expect(isOldModel("claude-opus-5")).toBe(false);
    expect(isOldModel("claude-opus-4-8")).toBe(false);
    expect(isOldModel("claude-sonnet-5")).toBe(false);
    expect(isOldModel("claude-sonnet-latest")).toBe(false);
    expect(isOldModel("claude-haiku-4-5-20251001")).toBe(false);
  });

  test("filters older Gemini and Mistral families", () => {
    expect(isOldModel("gemini-2.5-pro")).toBe(true);
    expect(isOldModel("gemini-3-flash-preview")).toBe(true);
    expect(isOldModel("mistral-small-2506")).toBe(true);
    expect(isOldModel("mistral-medium-3.1")).toBe(true);
    expect(isOldModel("mistral-medium-2508")).toBe(true);
    expect(isOldModel("devstral-2512")).toBe(true);
    expect(isOldModel("magistral-medium-2509")).toBe(true);

    expect(isOldModel("gemini-3.8-flash")).toBe(false);
    expect(isOldModel("gemini-3.6-flash")).toBe(false);
    expect(isOldModel("magistral-small-latest")).toBe(false);
    expect(isOldModel("gemini-3.5-flash-lite")).toBe(false);
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
        "anthropic/claude-opus-5",
        "anthropic/claude-sonnet-5",
        "openai/gpt-5.5",
        "google/gemini-3.6-flash",
        "google/gemini-3.8-flash",
        "google/gemini-3.5-flash-lite",
        "openai/chat-latest",
        "anthropic/claude-fable-5.1",
        "openai/gpt-5.6-sol",
        "x-ai/grok-4.6",
      ]),
    ).toEqual([
      "openai/gpt-5.6-sol",
      "openai/gpt-5.5",
      "openai/chat-latest",
      "anthropic/claude-fable-5.1",
      "anthropic/claude-opus-5",
      "anthropic/claude-sonnet-5",
      "openai/gpt-5.4-mini",
      "google/gemini-3.8-flash",
      "google/gemini-3.6-flash",
      "google/gemini-3.5-flash-lite",
      "x-ai/grok-4.6",
    ]);
  });

  test("prioritizes current models with dotted Bedrock prefixes", () => {
    expect(
      sortModelsByRecency(["custom-model", "anthropic.claude-sonnet-5"]),
    ).toEqual(["anthropic.claude-sonnet-5", "custom-model"]);
  });
});
