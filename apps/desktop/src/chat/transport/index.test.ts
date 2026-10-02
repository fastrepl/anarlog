import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  agentStream: vi.fn(),
  smoothStream: vi.fn(),
  streamTransform: vi.fn(),
  agentOptions: vi.fn(),
  renderTemplate: vi.fn(),
}));

vi.mock("@anlg/plugin-template", () => ({
  commands: { render: mocks.renderTemplate },
}));

vi.mock("ai", async (importOriginal) => ({
  ...(await importOriginal<typeof import("ai")>()),
  smoothStream: mocks.smoothStream,
  ToolLoopAgent: class {
    constructor(options: unknown) {
      mocks.agentOptions(options);
    }
    stream = mocks.agentStream;
  },
}));

import type { ContextRef } from "../context/entities";
import { MAX_TOOL_STEPS, MESSAGE_WINDOW_THRESHOLD } from "./helpers";
import { CustomChatTransport } from "./index";

describe("CustomChatTransport", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.smoothStream.mockReturnValue(mocks.streamTransform);
    mocks.agentStream.mockResolvedValue({
      toUIMessageStream: vi.fn(
        () =>
          new ReadableStream({
            start(controller) {
              controller.close();
            },
          }),
      ),
    });
  });

  it("streams chat responses word by word instead of waiting for newlines", async () => {
    const transport = new CustomChatTransport({} as never, {});

    await transport.sendMessages({
      abortSignal: new AbortController().signal,
      chatId: "chat-1",
      messageId: undefined,
      messages: [
        {
          id: "user-1",
          role: "user",
          parts: [{ type: "text", text: "Summarize this meeting" }],
        },
      ],
      trigger: "submit-message",
    });

    expect(mocks.smoothStream).toHaveBeenCalledWith({
      chunking: "word",
    });
    expect(mocks.agentStream).toHaveBeenCalledWith(
      expect.objectContaining({
        experimental_transform: mocks.streamTransform,
      }),
    );
  });

  it("reserves a final report step and retains the request and completed batches", async () => {
    const transport = new CustomChatTransport({} as never, {});
    await transport.sendMessages({
      chatId: "folder-chat",
      abortSignal: undefined,
      messageId: undefined,
      messages: [
        {
          id: "organize",
          role: "user",
          parts: [
            { type: "text", text: "Move every DEFCON 1 meeting into defcons" },
          ],
        },
      ],
      trigger: "submit-message",
    });
    const { stopWhen, prepareStep } = mocks.agentOptions.mock.calls[0]![0];
    expect(
      await stopWhen({
        steps: Array.from({ length: MAX_TOOL_STEPS - 1 }, () => ({})),
      }),
    ).toBe(false);
    expect(
      await stopWhen({
        steps: Array.from({ length: MAX_TOOL_STEPS }, () => ({})),
      }),
    ).toBe(true);

    await expect(
      prepareStep({ messages: [], stepNumber: MAX_TOOL_STEPS - 1 }),
    ).resolves.toMatchObject({
      toolChoice: "none",
      system: expect.stringContaining("next unprocessed offset"),
    });
    await expect(
      prepareStep({ messages: [], stepNumber: MAX_TOOL_STEPS - 2 }),
    ).resolves.toEqual({});

    const currentTurn = [
      { role: "user", content: "Move every DEFCON 1 meeting into defcons" },
      ...Array.from({ length: MESSAGE_WINDOW_THRESHOLD }, (_, index) => ({
        role: index % 2 === 0 ? "assistant" : "tool",
        content: `Batch ${index}`,
      })),
    ];
    await expect(
      prepareStep({
        stepNumber: 1,
        messages: [
          { role: "user", content: "Old conversation" },
          ...currentTurn,
        ],
      }),
    ).resolves.toEqual({ messages: currentTurn });
  });

  it("keeps earlier notes, marks the note from the latest message as current, and pins tools to it", async () => {
    mocks.renderTemplate.mockResolvedValue({ status: "ok", data: "CONTEXT" });
    const resolveContextRef = vi.fn(async (ref: ContextRef) =>
      ref.kind === "session"
        ? {
            kind: "session" as const,
            context: {
              sessionId: ref.sessionId,
              title: ref.sessionId,
              date: null,
              rawContent: null,
              enhancedContent: null,
              meetingChat: null,
              transcript: null,
              participants: [],
              event: null,
            },
          }
        : null,
    );
    const transport = new CustomChatTransport(
      {} as never,
      {},
      undefined,
      resolveContextRef,
    );
    const autoRef = (sessionId: string): ContextRef => ({
      kind: "session",
      key: `session:auto:${sessionId}`,
      source: "auto-current",
      sessionId,
    });

    await transport.sendMessages({
      chatId: "chat-1",
      abortSignal: undefined,
      messageId: undefined,
      messages: [
        {
          id: "user-new-first",
          role: "user",
          parts: [{ type: "text", text: "q about new" }],
          metadata: { contextRefs: [autoRef("new")] },
        },
        {
          id: "user-old",
          role: "user",
          parts: [{ type: "text", text: "q about old" }],
          metadata: { contextRefs: [autoRef("old")] },
        },
        {
          id: "user-new",
          role: "user",
          parts: [{ type: "text", text: "q about this note" }],
          metadata: {
            contextRefs: [
              autoRef("new"),
              {
                kind: "session",
                key: "session:manual:new",
                source: "manual",
                sessionId: "new",
              },
            ],
          },
        },
      ],
      trigger: "submit-message",
    });

    expect(mocks.renderTemplate).toHaveBeenCalledWith({
      contextBlock: {
        contexts: [
          expect.objectContaining({ sessionId: "old" }),
          expect.objectContaining({ sessionId: "new" }),
        ],
        currentSessionId: "new",
      },
    });
    expect(mocks.agentOptions).toHaveBeenCalledWith(
      expect.objectContaining({
        experimental_context: { currentSessionId: "new" },
      }),
    );
  });
});
