import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  appendCapturedMeetingChatMessagesToRawMd,
  startMeetingChatCapture,
} from "./meeting-chat-capture";

const {
  appendRawNoteParagraphsMock,
  getRawNoteEditorContentMock,
  appendSessionRawNoteMock,
  captureMeetingChatMessagesMock,
  updateSessionMock,
} = vi.hoisted(() => ({
  appendRawNoteParagraphsMock: vi.fn(),
  getRawNoteEditorContentMock: vi.fn(),
  appendSessionRawNoteMock: vi.fn(),
  captureMeetingChatMessagesMock: vi.fn(),
  updateSessionMock: vi.fn(),
}));

vi.mock("@hypr/plugin-detect", () => ({
  commands: {
    captureMeetingChatMessages: captureMeetingChatMessagesMock,
  },
}));

vi.mock("~/editor-bridge/raw-note-registry", () => ({
  appendRawNoteParagraphs: appendRawNoteParagraphsMock,
  getRawNoteEditorContent: getRawNoteEditorContentMock,
}));

vi.mock("~/session/queries", () => ({
  appendSessionRawNote: appendSessionRawNoteMock,
  updateSession: updateSessionMock,
}));

const capturedMessage = {
  id: "msg-1",
  platform: "zoom" as const,
  surface: "native" as const,
  sender: "Ada",
  timestamp: "10:42 AM",
  text: "Here is the doc https://example.com/spec",
  links: ["https://example.com/spec"],
};

describe("appendCapturedMeetingChatMessagesToRawMd", () => {
  test("preserves the title slot and adds links as marks", () => {
    const rawMd = JSON.stringify({
      type: "doc",
      content: [
        { type: "heading", attrs: { level: 1 } },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Existing memo" }],
        },
      ],
    });

    const result = appendCapturedMeetingChatMessagesToRawMd(
      rawMd,
      [capturedMessage],
      new Set(),
    );
    const parsed = JSON.parse(result.rawMd);

    expect(result.appended).toBe(1);
    expect(parsed.content[0]).toEqual({
      type: "heading",
      attrs: { level: 1 },
    });
    expect(parsed.content.at(-1)).toEqual({
      type: "paragraph",
      content: [
        { type: "text", text: "[Zoom chat] 10:42 AM Ada: " },
        { type: "text", text: "Here is the doc " },
        {
          type: "text",
          text: "https://example.com/spec",
          marks: [
            {
              type: "link",
              attrs: { href: "https://example.com/spec" },
            },
          ],
        },
      ],
    });
  });

  test("deduplicates exact paragraphs without substring false positives", () => {
    const rawMd = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "text",
              text: "Prefix [Zoom chat] 10:42 AM Ada: Here is the doc https://example.com/spec suffix",
            },
          ],
        },
      ],
    });

    const first = appendCapturedMeetingChatMessagesToRawMd(
      rawMd,
      [capturedMessage],
      new Set(),
    );
    const second = appendCapturedMeetingChatMessagesToRawMd(
      first.rawMd,
      [capturedMessage],
      new Set(),
    );

    expect(first.appended).toBe(1);
    expect(second.appended).toBe(0);
  });

  test("keeps distinct source messages that have identical visible text", () => {
    const result = appendCapturedMeetingChatMessagesToRawMd(
      JSON.stringify({ type: "doc", content: [] }),
      [capturedMessage, { ...capturedMessage, id: "msg-2" }],
      new Set(),
    );

    expect(result.appended).toBe(2);
  });
});

describe("startMeetingChatCapture", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    getRawNoteEditorContentMock.mockReturnValue(null);
    appendSessionRawNoteMock.mockImplementation(
      async (_sessionId: string, updater: (rawMd: string) => string | null) =>
        updater(JSON.stringify({ type: "doc", content: [] })) !== null,
    );
    updateSessionMock.mockResolvedValue(undefined);
    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: { id: "us.zoom.xos", name: "Zoom" },
        platform: "zoom",
        surface: "native",
        messages: [capturedMessage],
        warnings: [],
      },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("scopes each poll to the active meeting app", async () => {
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      bundleIds: ["us.zoom.xos"],
    });
    await vi.runOnlyPendingTimersAsync();
    stop();

    expect(captureMeetingChatMessagesMock).toHaveBeenCalledWith([
      "us.zoom.xos",
    ]);
    expect(appendSessionRawNoteMock).toHaveBeenCalledWith(
      "session-1",
      expect.any(Function),
    );
  });

  test("does not append a poll that resolves after capture stops", async () => {
    let resolveCapture: ((value: unknown) => void) | undefined;
    captureMeetingChatMessagesMock.mockReturnValue(
      new Promise((resolve) => {
        resolveCapture = resolve;
      }),
    );
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      bundleIds: ["us.zoom.xos"],
    });

    stop();
    resolveCapture?.({
      status: "ok",
      data: {
        app: { id: "us.zoom.xos", name: "Zoom" },
        platform: "zoom",
        surface: "native",
        messages: [capturedMessage],
        warnings: [],
      },
    });
    await Promise.resolve();
    await Promise.resolve();

    expect(appendSessionRawNoteMock).not.toHaveBeenCalled();
  });
});
