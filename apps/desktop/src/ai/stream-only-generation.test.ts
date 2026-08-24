import { wrapLanguageModel } from "ai";
import { describe, expect, test, vi } from "vitest";

import { streamOnlyGenerationMiddleware } from "./stream-only-generation";

type LanguageModel = Parameters<typeof wrapLanguageModel>[0]["model"];
type StreamPart =
  Awaited<
    ReturnType<LanguageModel["doStream"]>
  >["stream"] extends ReadableStream<infer Part>
    ? Part
    : never;

const usage = {
  inputTokens: {
    total: 4,
    noCache: 4,
    cacheRead: 0,
    cacheWrite: 0,
  },
  outputTokens: { total: 2, text: 2, reasoning: 0 },
};

describe("streamOnlyGenerationMiddleware", () => {
  test("uses streaming for generate calls and collects the result", async () => {
    const doGenerate = vi.fn(async () => {
      throw new Error("non-streaming request used");
    });
    const model = createModel(doGenerate, [
      { type: "stream-start", warnings: [] },
      {
        type: "response-metadata",
        id: "response-1",
        modelId: "gpt-test",
        timestamp: new Date("2026-08-25T00:00:00.000Z"),
      },
      { type: "text-start", id: "message-1" },
      { type: "text-delta", id: "message-1", delta: "Hello" },
      { type: "text-delta", id: "message-1", delta: " world" },
      { type: "text-end", id: "message-1" },
      {
        type: "tool-call",
        toolCallId: "tool-1",
        toolName: "lookup",
        input: '{"query":"test"}',
      },
      {
        type: "finish",
        finishReason: { unified: "stop", raw: "completed" },
        usage,
      },
    ]);
    const wrapped = wrapLanguageModel({
      model,
      middleware: streamOnlyGenerationMiddleware,
    });

    const result = await wrapped.doGenerate({ prompt: [] });

    expect(doGenerate).not.toHaveBeenCalled();
    expect(result.content).toEqual([
      { type: "text", text: "Hello world" },
      {
        type: "tool-call",
        toolCallId: "tool-1",
        toolName: "lookup",
        input: '{"query":"test"}',
      },
    ]);
    expect(result.finishReason).toEqual({ unified: "stop", raw: "completed" });
    expect(result.usage).toEqual(usage);
    expect(result.response).toMatchObject({
      headers: { "x-request-id": "request-1" },
      id: "response-1",
      modelId: "gpt-test",
    });
  });

  test("rejects provider errors from the response stream", async () => {
    const model = createModel(vi.fn(), [
      { type: "stream-start", warnings: [] },
      { type: "error", error: new Error("upstream failed") },
    ]);
    const wrapped = wrapLanguageModel({
      model,
      middleware: streamOnlyGenerationMiddleware,
    });

    await expect(wrapped.doGenerate({ prompt: [] })).rejects.toThrow(
      "upstream failed",
    );
  });
});

function createModel(
  doGenerate: LanguageModel["doGenerate"],
  parts: StreamPart[],
): LanguageModel {
  return {
    specificationVersion: "v3",
    provider: "test",
    modelId: "gpt-test",
    supportedUrls: {},
    doGenerate,
    doStream: async () => ({
      stream: new ReadableStream<StreamPart>({
        start(controller) {
          for (const part of parts) {
            controller.enqueue(part);
          }
          controller.close();
        },
      }),
      request: { body: { stream: true } },
      response: { headers: { "x-request-id": "request-1" } },
    }),
  };
}
