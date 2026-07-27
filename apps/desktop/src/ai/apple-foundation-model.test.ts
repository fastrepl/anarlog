import { describe, expect, it } from "vitest";

import { prepareFoundationModelRequest } from "./apple-foundation-model";

type CallOptions = Parameters<typeof prepareFoundationModelRequest>[0];

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

  it("adds JSON schema guidance and leaves oversized limits to the model", () => {
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

    expect(result.maximumResponseTokens).toBeNull();
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
});
