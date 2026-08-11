import { describe, expect, it } from "vitest";

import {
  getUnslothHeaders,
  getUnslothModelsUrl,
  processUnslothModels,
} from "./list-unsloth";

describe("getUnslothModelsUrl", () => {
  it("appends the models path to the OpenAI-compatible base URL", () => {
    expect(getUnslothModelsUrl("http://127.0.0.1:8888/v1")).toBe(
      "http://127.0.0.1:8888/v1/models",
    );
    expect(getUnslothModelsUrl("http://127.0.0.1:8888/v1/")).toBe(
      "http://127.0.0.1:8888/v1/models",
    );
  });
});

describe("getUnslothHeaders", () => {
  it("sends the bearer key only when one is saved", () => {
    expect(getUnslothHeaders("  sk-unsloth-abc  ")).toEqual({
      Authorization: "Bearer sk-unsloth-abc",
    });
    expect(getUnslothHeaders("   ")).toEqual({});
  });
});

describe("processUnslothModels", () => {
  it("keeps locally loaded GGUF models that hosted heuristics would drop", () => {
    const result = processUnslothModels([
      { id: "unsloth/Qwen3-235B-A22B-Instruct-2507-GGUF" },
      { id: "unsloth/gemma-3-27b-it-GGUF" },
    ]);

    expect(result.models).toEqual([
      "unsloth/gemma-3-27b-it-GGUF",
      "unsloth/Qwen3-235B-A22B-Instruct-2507-GGUF",
    ]);
    expect(result.ignored).toEqual([]);
  });

  it("ignores models that cannot serve chat", () => {
    const result = processUnslothModels([
      { id: "unsloth/gemma-3-27b-it-GGUF" },
      { id: "unsloth/embeddinggemma-300m-GGUF" },
      { id: "unsloth/whisper-large-v3-GGUF" },
    ]);

    expect(result.models).toEqual(["unsloth/gemma-3-27b-it-GGUF"]);
    expect(result.ignored).toEqual([
      { id: "unsloth/embeddinggemma-300m-GGUF", reasons: ["common_keyword"] },
      { id: "unsloth/whisper-large-v3-GGUF", reasons: ["common_keyword"] },
    ]);
  });
});
