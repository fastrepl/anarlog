import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  agentStream: vi.fn(),
  smoothStream: vi.fn(),
  streamTransform: vi.fn(),
  agentOptions: vi.fn(),
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

  it("allows a paginated action to continue beyond five steps and retains its request and completed batches", async () => {
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
      await stopWhen({ steps: Array.from({ length: 6 }, () => ({})) }),
    ).toBe(false);
    expect(
      await stopWhen({ steps: Array.from({ length: 20 }, () => ({})) }),
    ).toBe(true);

    const currentTurn = [
      { role: "user", content: "Move every DEFCON 1 meeting into defcons" },
      ...Array.from({ length: 22 }, (_, index) => ({
        role: index % 2 === 0 ? "assistant" : "tool",
        content: `Batch ${index}`,
      })),
    ];
    await expect(
      prepareStep({
        messages: [
          { role: "user", content: "Old conversation" },
          ...currentTurn,
        ],
      }),
    ).resolves.toEqual({ messages: currentTurn });
  });
});
