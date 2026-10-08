import type { LanguageModel } from "ai";
import { beforeEach, expect, it, vi } from "vitest";

import type { TaskArgsMapTransformed } from ".";
import { enhanceWorkflow } from "./enhance-workflow";

const mocks = vi.hoisted(() => ({ render: vi.fn(), streamText: vi.fn() }));
vi.mock("@anlg/plugin-template", () => ({
  commands: { render: mocks.render },
}));
vi.mock("ai", async (importOriginal) => ({
  ...(await importOriginal<typeof import("ai")>()),
  streamText: mocks.streamText,
}));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.render.mockImplementation(async (input) => ({
    status: "ok",
    data: input.enhanceSystem ? "Rendered system prompt" : "Meeting transcript",
  }));
  mocks.streamText.mockImplementation(() => ({
    fullStream: (async function* () {
      yield {
        type: "text-delta",
        id: "1",
        text: "The team agreed to ship Friday.",
      };
    })(),
  }));
});

it.each(["crisp", "balanced", "detailed"] as const)(
  "passes custom formatting to the renderer without adding layout rules in %s mode",
  async (summaryLength) => {
    const formatOverride =
      "Write an executive overview in prose, followed by discussion prose and bullets.";
    const args: TaskArgsMapTransformed["enhance"] = {
      language: "en",
      formatOverride,
      summaryLength,
      session: { title: "Launch", startedAt: null, endedAt: null, event: null },
      participants: [],
      template: null,
      preMeetingMemo: "",
      postMeetingMemo: "",
      transcripts: [
        {
          startedAt: null,
          endedAt: null,
          segments: [{ speaker: "John", text: "a".repeat(636), startMs: null }],
        },
      ],
      imageContext: [],
      lengthPolicy: {
        mode: summaryLength,
        transcript_characters: 636,
        guidance: {
          max_characters: 636,
          min_sections: 1,
          max_sections: 2,
        },
      },
      dictionaryTerms: [],
    };
    const chunks = [];
    for await (const chunk of enhanceWorkflow.executeWorkflow!({
      model: { provider: "openai.responses" } as LanguageModel,
      args,
      onProgress: vi.fn(),
      signal: new AbortController().signal,
    }))
      chunks.push(chunk);
    expect(chunks).toHaveLength(1);
    expect(mocks.streamText).toHaveBeenCalledTimes(1);
    const request = mocks.streamText.mock.calls[0][0];
    expect(mocks.render).toHaveBeenCalledWith({
      enhanceSystem: { language: "en", formatOverride },
    });
    expect(request.system).toContain("Rendered system prompt");
    expect(request.system).not.toMatch(
      /never put prose|bullets per section|# Next Steps/,
    );
    expect(request.prompt).toContain("Keep the requested structure");
    expect(request.prompt).not.toMatch(/\d to \d sections/);
    expect(request.maxOutputTokens).toBeUndefined();
  },
);

it("adds length guidance to prompts rendered from a template with sections", async () => {
  const args: TaskArgsMapTransformed["enhance"] = {
    language: "en",
    formatOverride: "",
    summaryLength: "detailed",
    session: { title: "Launch", startedAt: null, endedAt: null, event: null },
    participants: [],
    template: {
      title: "1:1 Meeting",
      description: null,
      sections: [
        { title: "Updates", description: "" },
        { title: "Next Steps", description: "" },
      ],
    },
    preMeetingMemo: "",
    postMeetingMemo: "",
    transcripts: [
      {
        startedAt: null,
        endedAt: null,
        segments: [
          { speaker: "John", text: "a".repeat(10_000), startMs: null },
        ],
      },
    ],
    imageContext: [],
    lengthPolicy: {
      mode: "detailed",
      transcript_characters: 10_000,
      guidance: {
        max_characters: 10_000,
        min_sections: 3,
        max_sections: 6,
      },
    },
    dictionaryTerms: [],
  };
  for await (const _ of enhanceWorkflow.executeWorkflow!({
    model: { provider: "anthropic.messages" } as LanguageModel,
    args,
    onProgress: vi.fn(),
    signal: new AbortController().signal,
  })) {
    // drain the stream
  }
  const request = mocks.streamText.mock.calls[0][0];
  expect(request.prompt).toContain("Summary length:");
  expect(request.prompt).toContain("Keep every requested template section");
  expect(request.prompt).not.toMatch(/\d to \d sections/);
  expect(request.maxOutputTokens).toBe(64_000);
});
