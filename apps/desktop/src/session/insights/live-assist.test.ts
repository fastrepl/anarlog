import { NoObjectGeneratedError } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { LiveTranscriptSegment } from "@anlg/plugin-transcription";

import {
  appendLiveAssistMarkdown,
  buildLiveAssistWindow,
  buildSummarizeSoFarInput,
  formatLiveAssistCardMarkdown,
  formatLiveAssistSpeakerLabel,
  sanitizeLiveAssistItem,
  sanitizeLiveAssistItems,
  streamLiveAssistSuggestion,
  truncateLiveAssistText,
} from "./live-assist";

const hoisted = vi.hoisted(() => ({
  renderCustom: vi.fn(async (_template: string, ctx: unknown) => ({
    status: "ok" as const,
    data: JSON.stringify(ctx),
  })),
  streamText: vi.fn(),
}));

vi.mock("ai", async (importOriginal) => ({
  ...(await importOriginal<typeof import("ai")>()),
  streamText: hoisted.streamText,
}));

vi.mock("@anlg/plugin-template", () => ({
  commands: {
    renderCustom: hoisted.renderCustom,
  },
}));

function makeSegment(
  overrides: Partial<LiveTranscriptSegment> &
    Pick<LiveTranscriptSegment, "id" | "start_ms" | "end_ms" | "text">,
): LiveTranscriptSegment {
  return {
    key: { channel: "RemoteParty", speaker_index: 0 },
    words: [],
    ...overrides,
  };
}

describe("formatLiveAssistSpeakerLabel", () => {
  it("labels the mic channel as You", () => {
    expect(formatLiveAssistSpeakerLabel({ channel: "DirectMic" })).toBe("You");
  });

  it("labels a diarized remote speaker by index", () => {
    expect(
      formatLiveAssistSpeakerLabel({
        channel: "RemoteParty",
        speaker_index: 2,
      }),
    ).toBe("Speaker 3");
  });

  it("falls back to a generic label without an index", () => {
    expect(formatLiveAssistSpeakerLabel({ channel: "MixedCapture" })).toBe(
      "Speaker",
    );
  });
});

describe("buildLiveAssistWindow", () => {
  it("keeps only segments overlapping the window, sorted and labeled", () => {
    const segments = [
      makeSegment({
        id: "b",
        start_ms: 20_000,
        end_ms: 25_000,
        text: "Second thing.",
        key: { channel: "DirectMic" },
      }),
      makeSegment({
        id: "old",
        start_ms: 0,
        end_ms: 1_000,
        text: "Too old to matter.",
      }),
      makeSegment({
        id: "a",
        start_ms: 10_000,
        end_ms: 15_000,
        text: "First thing.",
        key: { channel: "RemoteParty", speaker_index: 1 },
      }),
    ];

    expect(buildLiveAssistWindow(segments, 5_000, 30_000, 4_000)).toBe(
      "Speaker 2: First thing.\nYou: Second thing.",
    );
  });

  it("drops blank segments", () => {
    const segments = [
      makeSegment({ id: "a", start_ms: 0, end_ms: 1, text: "   " }),
    ];
    expect(buildLiveAssistWindow(segments, 0, 10_000, 4_000)).toBe("");
  });
});

describe("buildSummarizeSoFarInput", () => {
  it("renders speaker/text lines", () => {
    expect(
      buildSummarizeSoFarInput(
        {
          segments: [
            { speaker: "Ada", text: "Let's confirm the launch date." },
            { speaker: "You", text: "I will send the doc today." },
          ],
        },
        4_000,
      ),
    ).toBe(
      "Ada: Let's confirm the launch date.\nYou: I will send the doc today.",
    );
  });

  it("returns an empty string without a transcript", () => {
    expect(buildSummarizeSoFarInput(null, 4_000)).toBe("");
    expect(buildSummarizeSoFarInput({ segments: [] }, 4_000)).toBe("");
  });
});

describe("truncateLiveAssistText", () => {
  it("keeps short text untouched", () => {
    expect(truncateLiveAssistText("short", 100)).toBe("short");
  });

  it("keeps the end of the text and cuts on a nearby line boundary", () => {
    const text = "line one\nline two\nline three";
    expect(truncateLiveAssistText(text, 20)).toBe("line two\nline three");
  });

  it("hard-cuts when no clean boundary is close enough", () => {
    const text = "a".repeat(50);
    expect(truncateLiveAssistText(text, 10)).toBe("a".repeat(10));
  });
});

describe("sanitizeLiveAssistItem / sanitizeLiveAssistItems", () => {
  it("strips leading bullets, numbering and emphasis", () => {
    expect(sanitizeLiveAssistItem("- **Ship the doc.**")).toBe("Ship the doc.");
    expect(sanitizeLiveAssistItem("2) Follow up with Ada")).toBe(
      "Follow up with Ada",
    );
  });

  it("drops empty items, dedupes case-insensitively, and caps at three", () => {
    expect(
      sanitizeLiveAssistItems([
        "- Ship the doc.",
        "",
        "ship the doc.",
        "Confirm the date.",
        "Send the invite.",
        "Extra item that should be dropped.",
      ]),
    ).toEqual(["Ship the doc.", "Confirm the date.", "Send the invite."]);
  });
});

describe("appendLiveAssistMarkdown / formatLiveAssistCardMarkdown", () => {
  it("formats a labeled bullet block", () => {
    expect(
      formatLiveAssistCardMarkdown("Action Items", [
        "Ship the doc.",
        "Confirm the date.",
      ]),
    ).toBe("**Action Items**\n\n- Ship the doc.\n- Confirm the date.");
  });

  it("appends after existing notes instead of replacing them", () => {
    expect(appendLiveAssistMarkdown("Existing notes", "## Addition")).toBe(
      "Existing notes\n\n## Addition",
    );
    expect(appendLiveAssistMarkdown("", "## Addition")).toBe("## Addition");
    expect(appendLiveAssistMarkdown("Existing notes", "")).toBe(
      "Existing notes",
    );
  });
});

describe("streamLiveAssistSuggestion", () => {
  beforeEach(() => {
    hoisted.renderCustom.mockClear();
    hoisted.streamText.mockReset();
  });

  it("returns no items for empty source text without calling the model", async () => {
    const items = await streamLiveAssistSuggestion({
      model: { id: "model-1" } as never,
      language: "en",
      kind: "catch_up",
      sourceText: "   ",
    });

    expect(items).toEqual([]);
    expect(hoisted.streamText).not.toHaveBeenCalled();
  });

  it("sanitizes the structured items from the model", async () => {
    hoisted.streamText.mockReturnValue({
      output: Promise.resolve({
        items: ["- Ship the doc.", "Confirm the date."],
      }),
    });

    const items = await streamLiveAssistSuggestion({
      model: { id: "model-1" } as never,
      language: "en",
      kind: "action_items",
      sourceText: "You: We should ship the doc.",
    });

    expect(items).toEqual(["Ship the doc.", "Confirm the date."]);
    expect(hoisted.streamText).toHaveBeenCalledWith(
      expect.objectContaining({
        model: { id: "model-1" },
        maxRetries: 1,
        maxOutputTokens: 260,
      }),
    );
  });

  it("falls back to extracting bullet lines when the model ignores the schema", async () => {
    const error = new NoObjectGeneratedError({
      text: "- Follow up with Ada.\n- Confirm the launch date.",
      response: {},
      usage: {},
      finishReason: "stop",
    } as never);
    hoisted.streamText.mockReturnValue({
      get output() {
        return Promise.reject(error);
      },
    });

    await expect(
      streamLiveAssistSuggestion({
        model: { id: "model-1" } as never,
        language: "en",
        kind: "follow_up",
        sourceText: "You: We should ship the doc.",
      }),
    ).resolves.toEqual(["Follow up with Ada.", "Confirm the launch date."]);
  });

  it("rethrows when no items can be recovered from the raw text", async () => {
    const error = new NoObjectGeneratedError({
      text: "",
      response: {},
      usage: {},
      finishReason: "stop",
    } as never);
    hoisted.streamText.mockReturnValue({
      get output() {
        return Promise.reject(error);
      },
    });

    await expect(
      streamLiveAssistSuggestion({
        model: { id: "model-1" } as never,
        language: "en",
        kind: "follow_up",
        sourceText: "You: We should ship the doc.",
      }),
    ).rejects.toBe(error);
  });
});
