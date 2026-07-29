import { beforeEach, describe, expect, it, vi } from "vitest";

const localLlmCommands = vi.hoisted(() => ({
  foundationModelBegin: vi.fn(),
  foundationModelCancel: vi.fn(),
  foundationModelGenerate: vi.fn(),
}));

vi.mock("@anlg/plugin-local-llm", () => ({ commands: localLlmCommands }));

import {
  createAppleFoundationModel,
  prepareFoundationModelRequest,
} from "./apple-foundation-model";

type CallOptions = Parameters<typeof prepareFoundationModelRequest>[0];

beforeEach(() => {
  localLlmCommands.foundationModelBegin.mockReset();
  localLlmCommands.foundationModelCancel.mockReset();
  localLlmCommands.foundationModelGenerate.mockReset();
  localLlmCommands.foundationModelBegin.mockResolvedValue({
    status: "ok",
    data: null,
  });
  localLlmCommands.foundationModelCancel.mockResolvedValue({
    status: "ok",
    data: null,
  });
});

describe("prepareFoundationModelRequest", () => {
  it("keeps system instructions separate from a simple user prompt", () => {
    const result = prepareFoundationModelRequest({
      prompt: [
        { role: "system", content: "Summarize precisely." },
        {
          role: "user",
          content: [{ type: "text", text: "Meeting transcript" }],
        },
      ],
      temperature: 0,
      maxOutputTokens: 128,
    });

    expect(result).toEqual({
      instructions: "Summarize precisely.",
      prompt: "Meeting transcript",
      maximumResponseTokens: 128,
      temperature: 0,
      useGreedySampling: true,
    });
  });

  it("labels multi-turn conversation history", () => {
    const result = prepareFoundationModelRequest({
      prompt: [
        {
          role: "user",
          content: [{ type: "text", text: "What was decided?" }],
        },
        {
          role: "assistant",
          content: [{ type: "text", text: "The launch moved." }],
        },
        {
          role: "user",
          content: [{ type: "text", text: "To when?" }],
        },
      ],
    });

    expect(result.prompt).toBe(
      "USER:\nWhat was decided?\n\nASSISTANT:\nThe launch moved.\n\nUSER:\nTo when?",
    );
  });

  it("adds JSON schema guidance and caps oversized output limits", () => {
    const result = prepareFoundationModelRequest({
      prompt: [
        {
          role: "user",
          content: [{ type: "text", text: "Extract facts." }],
        },
      ],
      maxOutputTokens: 8192,
      responseFormat: {
        type: "json",
        schema: {
          type: "object",
          properties: { facts: { type: "array", items: { type: "string" } } },
        },
      },
    });

    expect(result.maximumResponseTokens).toBe(4096);
    expect(result.prompt).toContain("Return only valid JSON");
    expect(result.prompt).toContain('"facts"');
  });

  it("rejects image input for the text-only experiment", () => {
    expect(() =>
      prepareFoundationModelRequest({
        prompt: [
          {
            role: "user",
            content: [
              {
                type: "file",
                data: "image-data",
                mediaType: "image/png",
              },
            ],
          },
        ],
      } as CallOptions),
    ).toThrow("currently supports text only");
  });

  it("cancels and drains native generation when the request is aborted", async () => {
    const controller = new AbortController();
    const reason = new Error("stop generation");
    let finish:
      | ((value: { status: "error"; error: string }) => void)
      | undefined;
    localLlmCommands.foundationModelGenerate.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );

    const generation = createAppleFoundationModel().doGenerate({
      prompt: [
        {
          role: "user",
          content: [{ type: "text", text: "Summarize this." }],
        },
      ],
      abortSignal: controller.signal,
    } as CallOptions);
    await vi.waitFor(() =>
      expect(localLlmCommands.foundationModelGenerate).toHaveBeenCalled(),
    );
    const requestId = localLlmCommands.foundationModelBegin.mock.calls[0]?.[0];

    controller.abort(reason);
    await vi.waitFor(() =>
      expect(localLlmCommands.foundationModelCancel).toHaveBeenCalledWith(
        requestId,
      ),
    );
    finish?.({ status: "error", error: "Generation was cancelled." });

    await expect(generation).rejects.toBe(reason);
    expect(localLlmCommands.foundationModelGenerate).toHaveBeenCalledWith(
      expect.objectContaining({ requestId }),
    );
  });

  it("cancels after registration when abort wins the begin race", async () => {
    const controller = new AbortController();
    const reason = new Error("cancel before generation");
    let finishBegin:
      | ((value: { status: "ok"; data: null }) => void)
      | undefined;
    localLlmCommands.foundationModelBegin.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishBegin = resolve;
        }),
    );

    const generation = createAppleFoundationModel().doGenerate({
      prompt: [
        {
          role: "user",
          content: [{ type: "text", text: "Summarize this." }],
        },
      ],
      abortSignal: controller.signal,
    } as CallOptions);
    controller.abort(reason);
    finishBegin?.({ status: "ok", data: null });

    await expect(generation).rejects.toBe(reason);
    const requestId = localLlmCommands.foundationModelBegin.mock.calls[0]?.[0];
    expect(localLlmCommands.foundationModelCancel).toHaveBeenCalledWith(
      requestId,
    );
    expect(localLlmCommands.foundationModelGenerate).not.toHaveBeenCalled();
  });
});
