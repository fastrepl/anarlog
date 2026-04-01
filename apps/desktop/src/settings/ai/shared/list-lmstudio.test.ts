import { describe, expect, test } from "vitest";

import {
  getLMStudioNativeModelsUrl,
  processLMStudioModels,
} from "./list-lmstudio";

describe("getLMStudioNativeModelsUrl", () => {
  test("maps the OpenAI-compatible default URL to the native models endpoint", () => {
    expect(getLMStudioNativeModelsUrl("http://127.0.0.1:1234/v1")).toBe(
      "http://127.0.0.1:1234/api/v1/models",
    );
  });

  test("preserves reverse-proxy path prefixes", () => {
    expect(getLMStudioNativeModelsUrl("https://example.com/lmstudio/v1")).toBe(
      "https://example.com/lmstudio/api/v1/models",
    );
  });
});

describe("processLMStudioModels", () => {
  test("keeps loaded tool-capable models and sorts loaded models first", () => {
    expect(
      processLMStudioModels([
        {
          type: "llm",
          key: "second",
          loaded_instances: [],
          max_context_length: 32768,
          capabilities: {
            trained_for_tool_use: true,
            vision: false,
          },
        },
        {
          type: "llm",
          key: "first",
          loaded_instances: [{}],
          max_context_length: 32768,
          capabilities: {
            trained_for_tool_use: true,
            vision: true,
          },
        },
      ]),
    ).toEqual({
      models: ["first", "second"],
      ignored: [],
      metadata: {
        first: { input_modalities: ["text", "image"] },
        second: { input_modalities: ["text"] },
      },
    });
  });

  test("ignores embedding, no-tool, and small-context models", () => {
    expect(
      processLMStudioModels([
        {
          type: "embedding",
          key: "embedder",
          loaded_instances: [],
          max_context_length: 8192,
        },
        {
          type: "llm",
          key: "no-tools",
          loaded_instances: [],
          max_context_length: 32768,
          capabilities: {
            trained_for_tool_use: false,
          },
        },
        {
          type: "llm",
          key: "small-context",
          loaded_instances: [],
          max_context_length: 4096,
          capabilities: {
            trained_for_tool_use: true,
          },
        },
      ]),
    ).toEqual({
      models: [],
      ignored: [
        { id: "embedder", reasons: ["not_llm"] },
        { id: "no-tools", reasons: ["no_tool"] },
        { id: "small-context", reasons: ["context_too_small"] },
      ],
      metadata: {},
    });
  });
});
