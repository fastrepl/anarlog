import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { StrictMode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { PersistedChatMessage } from "~/chat/store/persisted-messages";

const mocks = vi.hoisted(() => ({
  chatRegenerate: vi.fn(),
  chatSendMessage: vi.fn(),
  chatSetMessages: vi.fn(),
  chatStop: vi.fn(),
  chatInits: [] as unknown[],
  beginCloudsyncActivity: vi.fn().mockResolvedValue(undefined),
  deleteChatMessage: vi.fn().mockResolvedValue(undefined),
  deleteChatMessagesExcept: vi.fn().mockResolvedValue(undefined),
  endCloudsyncActivity: vi.fn().mockResolvedValue(undefined),
  flushDatabaseWritesByPrefix: vi.fn().mockResolvedValue(undefined),
  getChatMessageGroupId: vi.fn().mockResolvedValue(null),
  messages: [] as unknown[],
  persistedMessages: [] as PersistedChatMessage[],
  replaceChatMessage: vi.fn().mockResolvedValue(undefined),
  status: "ready",
  store: {} as unknown,
  transport: {} as unknown,
  upsertChatMessage: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@anlg/plugin-db", () => ({
  beginCloudsyncActivity: mocks.beginCloudsyncActivity,
  endCloudsyncActivity: mocks.endCloudsyncActivity,
}));

vi.mock("~/db/write-queue", () => ({
  flushDatabaseWritesByPrefix: mocks.flushDatabaseWritesByPrefix,
}));

vi.mock("@ai-sdk/react", () => ({
  Chat: class MockChat {
    id: string;

    constructor(init: { id: string }) {
      this.id = init.id;
      mocks.chatInits.push(init);
    }
  },
  useChat: () => ({
    messages: mocks.messages,
    sendMessage: mocks.chatSendMessage,
    regenerate: mocks.chatRegenerate,
    stop: mocks.chatStop,
    status: mocks.status,
    error: undefined,
    setMessages: mocks.chatSetMessages,
  }),
}));

vi.mock("~/chat/context/use-chat-context-pipeline", () => ({
  useChatContextPipeline: () => ({
    contextEntities: [],
    pendingRefs: [],
  }),
}));

vi.mock("~/chat/store/queries", () => ({
  deleteChatMessage: mocks.deleteChatMessage,
  deleteChatMessagesExcept: mocks.deleteChatMessagesExcept,
  getChatMessageGroupId: mocks.getChatMessageGroupId,
  replaceChatMessage: mocks.replaceChatMessage,
  upsertChatMessage: mocks.upsertChatMessage,
  usePersistedChatMessages: () => mocks.persistedMessages,
}));

vi.mock("~/chat/transport/use-transport", () => ({
  useTransport: () => ({
    transport: mocks.transport,
    isSystemPromptReady: true,
  }),
}));

vi.mock("~/shared/owner-user", () => ({
  useOwnerUserId: () => "user-1",
}));

import { ChatSession, type ChatSessionRenderProps } from "./session-provider";

import {
  buildPersistedChatMessage,
  type ChatMessageRecord,
} from "~/chat/store/persisted-messages";
import type { AnlgUIMessage } from "~/chat/types";

function persistedMessage(
  message: AnlgUIMessage,
  chatGroupId = "group-1",
): PersistedChatMessage {
  const record: ChatMessageRecord = buildPersistedChatMessage({
    message,
    chatGroupId,
    ownerUserId: "user-1",
    status: "ready",
  });
  return { id: message.id, message, record, status: "ready" };
}

function renderSession() {
  return render(
    <ChatSession chatGroupId="group-1" sessionId="session-1">
      {({ regenerate }) => (
        <button type="button" onClick={regenerate}>
          Regenerate
        </button>
      )}
    </ChatSession>,
  );
}

function connectChatSendToTransport(
  sendMessages = vi
    .fn()
    .mockResolvedValue(
      new ReadableStream({ start: (controller) => controller.close() }),
    ),
) {
  mocks.transport = {
    sendMessages,
    reconnectToStream: vi.fn().mockResolvedValue(null),
  };
  mocks.chatSendMessage.mockImplementation((message: AnlgUIMessage) => {
    const init = mocks.chatInits[mocks.chatInits.length - 1] as {
      transport: {
        sendMessages: (options: {
          trigger: "submit-message";
          chatId: string;
          messageId: undefined;
          messages: AnlgUIMessage[];
          abortSignal: AbortSignal;
        }) => Promise<ReadableStream>;
      };
    };
    return init.transport.sendMessages({
      trigger: "submit-message",
      chatId: "session-1",
      messageId: undefined,
      messages: [message],
      abortSignal: new AbortController().signal,
    });
  });
  return sendMessages;
}

function connectChatRegenerateToTransport(
  sendMessages = vi
    .fn()
    .mockResolvedValue(
      new ReadableStream({ start: (controller) => controller.close() }),
    ),
) {
  mocks.transport = {
    sendMessages,
    reconnectToStream: vi.fn().mockResolvedValue(null),
  };
  mocks.chatRegenerate.mockImplementation(() => {
    const init = mocks.chatInits[mocks.chatInits.length - 1] as {
      transport: {
        sendMessages: (options: {
          trigger: "regenerate-message";
          chatId: string;
          messageId: string | undefined;
          messages: AnlgUIMessage[];
          abortSignal: AbortSignal;
        }) => Promise<ReadableStream>;
      };
    };
    return init.transport.sendMessages({
      trigger: "regenerate-message",
      chatId: "session-1",
      messageId: undefined,
      messages: mocks.messages as AnlgUIMessage[],
      abortSignal: new AbortController().signal,
    });
  });
  return sendMessages;
}

function startedNativeKey(callIndex = 0) {
  return mocks.beginCloudsyncActivity.mock.calls[callIndex]?.[1] as string;
}

describe("ChatSession", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.chatInits = [];
    mocks.messages = [];
    mocks.persistedMessages = [];
    mocks.status = "ready";
    mocks.store = {};
    mocks.transport = {};
    mocks.chatSendMessage.mockReset().mockResolvedValue(undefined);
    mocks.chatRegenerate.mockReset().mockResolvedValue(undefined);
    mocks.chatStop.mockReset().mockResolvedValue(undefined);
    mocks.beginCloudsyncActivity.mockReset().mockResolvedValue(undefined);
    mocks.deleteChatMessage.mockReset().mockResolvedValue(undefined);
    mocks.deleteChatMessagesExcept.mockReset().mockResolvedValue(undefined);
    mocks.endCloudsyncActivity.mockReset().mockResolvedValue(undefined);
    mocks.flushDatabaseWritesByPrefix.mockReset().mockResolvedValue(undefined);
    mocks.getChatMessageGroupId.mockReset().mockResolvedValue(null);
    mocks.replaceChatMessage.mockReset().mockResolvedValue(undefined);
    mocks.upsertChatMessage.mockReset().mockResolvedValue(undefined);
  });

  it("responds locally while batch transcription is pending", async () => {
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    render(
      <ChatSession
        chatGroupId="group-1"
        currentSessionId="session-1"
        isBatchTranscriptionPending
        sessionId="chat-1"
      >
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );
    const userCreatedAt = Date.now() + 60_000;
    const userMessage: AnlgUIMessage = {
      id: "user-message",
      role: "user",
      parts: [{ type: "text", text: "What did they say?" }],
      metadata: { createdAt: userCreatedAt },
    };
    let finishBeforeSend: (() => void) | undefined;
    const beforeSend = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finishBeforeSend = resolve;
        }),
    );

    captured.send?.(userMessage, {
      chatGroupId: "group-1",
      beforeSend,
    });

    expect(mocks.chatSendMessage).not.toHaveBeenCalled();
    await waitFor(() => expect(beforeSend).toHaveBeenCalledOnce());
    const setMessages = mocks.chatSetMessages.mock.calls.find(
      ([next]) => typeof next === "function",
    )?.[0] as ((current: AnlgUIMessage[]) => AnlgUIMessage[]) | undefined;
    expect(setMessages?.([])).toEqual([
      userMessage,
      expect.objectContaining({ role: "assistant" }),
    ]);

    finishBeforeSend?.();
    await waitFor(() => expect(mocks.upsertChatMessage).toHaveBeenCalledOnce());
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        chatGroupId: "group-1",
        role: "assistant",
        createdAt: new Date(userCreatedAt + 1).toISOString(),
        content:
          "This recording is using batch transcription, so the transcript isn't available to chat yet. Ask again after transcription finishes, or switch to a Pro model for live transcription.",
      }),
    );
  });

  it("uses an existing transcript while batch retranscription is pending", () => {
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    render(
      <ChatSession
        chatGroupId="group-1"
        currentSessionId="session-1"
        hasAvailableTranscript
        isBatchTranscriptionPending
        sessionId="chat-1"
      >
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );
    const userMessage: AnlgUIMessage = {
      id: "user-message",
      role: "user",
      parts: [{ type: "text", text: "What did they say?" }],
    };

    captured.send?.(userMessage, { chatGroupId: "group-1" });

    expect(mocks.chatSendMessage).toHaveBeenCalledWith(userMessage);
    expect(mocks.chatSetMessages).not.toHaveBeenCalled();
  });

  it("removes local batch guidance when persistence fails", async () => {
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    render(
      <ChatSession
        chatGroupId="group-1"
        currentSessionId="session-1"
        isBatchTranscriptionPending
        sessionId="chat-1"
      >
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );
    const userMessage: AnlgUIMessage = {
      id: "failed-user-message",
      role: "user",
      parts: [{ type: "text", text: "What did they say?" }],
    };

    captured.send?.(userMessage, {
      chatGroupId: "group-1",
      beforeSend: vi.fn().mockRejectedValue(new Error("create failed")),
    });

    await waitFor(() => expect(consoleError).toHaveBeenCalledOnce());
    const updates = mocks.chatSetMessages.mock.calls
      .map(([next]) => next)
      .filter(
        (next): next is (current: AnlgUIMessage[]) => AnlgUIMessage[] =>
          typeof next === "function",
      );
    const optimisticMessages = updates[0]?.([]) ?? [];
    expect(optimisticMessages).toEqual([
      userMessage,
      expect.objectContaining({ role: "assistant" }),
    ]);
    expect(updates[1]?.(optimisticMessages)).toEqual([]);
    expect(mocks.deleteChatMessage).toHaveBeenCalledTimes(2);
    expect(mocks.deleteChatMessage).toHaveBeenCalledWith(
      "group-1",
      "failed-user-message",
    );
    expect(mocks.chatSendMessage).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  it("deletes the persisted user turn when batch guidance fails to save", async () => {
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    mocks.upsertChatMessage.mockRejectedValueOnce(
      new Error("assistant save failed"),
    );
    render(
      <ChatSession
        chatGroupId="group-1"
        currentSessionId="session-1"
        isBatchTranscriptionPending
        sessionId="chat-1"
      >
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );

    captured.send?.(
      {
        id: "persisted-user-message",
        role: "user",
        parts: [{ type: "text", text: "What did they say?" }],
      },
      {
        chatGroupId: "group-1",
        beforeSend: vi.fn().mockResolvedValue(undefined),
      },
    );

    await waitFor(() => expect(consoleError).toHaveBeenCalledOnce());
    expect(mocks.deleteChatMessage).toHaveBeenCalledTimes(2);
    expect(mocks.deleteChatMessage).toHaveBeenCalledWith(
      "group-1",
      "persisted-user-message",
    );
    expect(mocks.chatSendMessage).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  it("does not regenerate against a pending batch transcript", () => {
    render(
      <ChatSession
        chatGroupId="group-1"
        isBatchTranscriptionPending
        sessionId="chat-1"
      >
        {({ regenerate }) => (
          <button type="button" onClick={regenerate}>
            Regenerate
          </button>
        )}
      </ChatSession>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    expect(mocks.chatRegenerate).not.toHaveBeenCalled();
  });

  it("regenerates from an existing transcript during batch retranscription", () => {
    mocks.messages = [
      {
        id: "user-message",
        role: "user",
        parts: [{ type: "text", text: "What did they say?" }],
      },
    ];
    render(
      <ChatSession
        chatGroupId="group-1"
        hasAvailableTranscript
        isBatchTranscriptionPending
        sessionId="chat-1"
      >
        {({ regenerate }) => (
          <button type="button" onClick={regenerate}>
            Regenerate
          </button>
        )}
      </ChatSession>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    expect(mocks.chatRegenerate).toHaveBeenCalledOnce();
  });

  it.each([
    {
      name: "empty",
      id: "assistant-new",
      parts: [] as AnlgUIMessage["parts"],
      isAbort: false,
      isError: false,
      shouldReplace: false,
    },
    {
      name: "aborted",
      id: "assistant-new",
      parts: [
        { type: "text", text: "Partial answer" },
      ] as AnlgUIMessage["parts"],
      isAbort: true,
      isError: false,
      shouldReplace: false,
    },
    {
      name: "failed",
      id: "assistant-new",
      parts: [
        { type: "text", text: "Partial answer" },
      ] as AnlgUIMessage["parts"],
      isAbort: false,
      isError: true,
      shouldReplace: false,
    },
    {
      name: "failed-same-id",
      id: "assistant-old",
      parts: [
        { type: "text", text: "Partial answer" },
      ] as AnlgUIMessage["parts"],
      isAbort: false,
      isError: true,
      shouldReplace: false,
    },
    {
      name: "same-id",
      id: "assistant-old",
      parts: [
        { type: "text", text: "Replacement answer" },
      ] as AnlgUIMessage["parts"],
      isAbort: false,
      isError: false,
      shouldReplace: true,
    },
  ])(
    "keeps the prior assistant for a $name replacement",
    async ({ id, parts, isAbort, isError, shouldReplace }) => {
      const userMessage: AnlgUIMessage = {
        id: "user-1",
        role: "user",
        parts: [{ type: "text", text: "Question" }],
      };
      const oldAssistant: AnlgUIMessage = {
        id: "assistant-old",
        role: "assistant",
        parts: [{ type: "text", text: "Old answer" }],
      };
      const replacement: AnlgUIMessage = {
        id,
        role: "assistant",
        parts,
      };
      mocks.messages = [userMessage, oldAssistant];
      mocks.getChatMessageGroupId.mockResolvedValue("group-1");
      const sendTransport = connectChatRegenerateToTransport();

      renderSession();
      fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
      await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());
      const chat = mocks.chatInits[0] as {
        onFinish: (params: {
          message: AnlgUIMessage;
          messages: AnlgUIMessage[];
          isAbort: boolean;
          isError: boolean;
        }) => void;
      };
      chat.onFinish({
        isAbort,
        isError,
        message: replacement,
        messages: [userMessage, replacement],
      });

      if (shouldReplace) {
        await waitFor(() =>
          expect(mocks.replaceChatMessage).toHaveBeenCalledWith({
            message: expect.objectContaining({ id: replacement.id }),
            previousMessageId: oldAssistant.id,
          }),
        );
      } else {
        await Promise.resolve();
        await Promise.resolve();
        expect(mocks.replaceChatMessage).not.toHaveBeenCalled();
      }
      expect(mocks.upsertChatMessage).not.toHaveBeenCalledWith(
        expect.objectContaining({ id: replacement.id }),
      );
    },
  );

  it("keeps persisting partial failures for normal sends", async () => {
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const partialAssistant: AnlgUIMessage = {
      id: "assistant-partial",
      role: "assistant",
      parts: [{ type: "text", text: "Partial answer" }],
    };
    mocks.messages = [userMessage];
    mocks.getChatMessageGroupId.mockResolvedValue("group-1");

    renderSession();
    const chat = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
        isError: boolean;
      }) => void;
    };
    chat.onFinish({
      isAbort: false,
      isError: true,
      message: partialAssistant,
      messages: [userMessage, partialAssistant],
    });

    await waitFor(() =>
      expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
        expect.objectContaining({ id: partialAssistant.id }),
      ),
    );
    expect(mocks.replaceChatMessage).not.toHaveBeenCalled();
  });

  it("does not replace an assistant from before the latest user", async () => {
    const firstUser: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "First question" }],
    };
    const firstAssistant: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "First answer" }],
    };
    const latestUser: AnlgUIMessage = {
      id: "user-2",
      role: "user",
      parts: [{ type: "text", text: "Second question" }],
    };
    const latestAssistant: AnlgUIMessage = {
      id: "assistant-2",
      role: "assistant",
      parts: [{ type: "text", text: "Second answer" }],
    };
    mocks.status = "error";
    mocks.messages = [firstUser, firstAssistant, latestUser];
    mocks.getChatMessageGroupId.mockResolvedValue("group-1");
    const sendTransport = connectChatRegenerateToTransport();

    renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());
    const chat = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
        isError: boolean;
      }) => void;
    };
    chat.onFinish({
      isAbort: false,
      isError: false,
      message: latestAssistant,
      messages: [firstUser, firstAssistant, latestUser, latestAssistant],
    });

    await waitFor(() =>
      expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
        expect.objectContaining({ id: latestAssistant.id }),
      ),
    );
    expect(mocks.replaceChatMessage).not.toHaveBeenCalled();
    expect(mocks.deleteChatMessage).not.toHaveBeenCalledWith(
      "group-1",
      firstAssistant.id,
    );
  });

  it.each([
    {
      name: "partial error",
      failedMessage: {
        id: "assistant-failed",
        role: "assistant" as const,
        parts: [{ type: "text" as const, text: "Partial answer" }],
      },
      isAbort: false,
      isError: true,
    },
    {
      name: "empty response",
      failedMessage: {
        id: "assistant-empty",
        role: "assistant" as const,
        parts: [],
      },
      isAbort: false,
      isError: false,
    },
    {
      name: "aborted response",
      failedMessage: {
        id: "assistant-aborted",
        role: "assistant" as const,
        parts: [{ type: "text" as const, text: "Partial answer" }],
      },
      isAbort: true,
      isError: false,
    },
  ])(
    "keeps the original durable target across a $name retry",
    async ({ failedMessage, isAbort, isError }) => {
      const userMessage: AnlgUIMessage = {
        id: "user-1",
        role: "user",
        parts: [{ type: "text", text: "Question" }],
      };
      const oldAssistant: AnlgUIMessage = {
        id: "assistant-old",
        role: "assistant",
        parts: [{ type: "text", text: "Old answer" }],
      };
      const replacement: AnlgUIMessage = {
        id: "assistant-new",
        role: "assistant",
        parts: [{ type: "text", text: "Replacement answer" }],
      };
      mocks.messages = [userMessage, oldAssistant];
      mocks.getChatMessageGroupId.mockResolvedValue("group-1");
      const sendTransport = connectChatRegenerateToTransport();

      const view = renderSession();
      fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
      await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());
      const chat = mocks.chatInits[0] as {
        onFinish: (params: {
          message: AnlgUIMessage;
          messages: AnlgUIMessage[];
          isAbort: boolean;
          isError: boolean;
        }) => void;
      };
      chat.onFinish({
        isAbort,
        isError,
        message: failedMessage,
        messages: [userMessage, failedMessage],
      });
      await Promise.resolve();
      expect(mocks.replaceChatMessage).not.toHaveBeenCalled();
      expect(mocks.upsertChatMessage).not.toHaveBeenCalledWith(
        expect.objectContaining({ id: failedMessage.id }),
      );

      mocks.messages = failedMessage.parts.length
        ? [userMessage, failedMessage]
        : [userMessage];
      view.rerender(
        <ChatSession chatGroupId="group-1" sessionId="session-1">
          {({ regenerate }) => (
            <button type="button" onClick={regenerate}>
              Regenerate
            </button>
          )}
        </ChatSession>,
      );
      await new Promise((resolve) => setTimeout(resolve, 0));
      fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
      await waitFor(() => expect(sendTransport).toHaveBeenCalledTimes(2));
      chat.onFinish({
        isAbort: false,
        isError: false,
        message: replacement,
        messages: [userMessage, replacement],
      });

      await waitFor(() =>
        expect(mocks.replaceChatMessage).toHaveBeenCalledWith({
          message: expect.objectContaining({ id: replacement.id }),
          previousMessageId: oldAssistant.id,
        }),
      );
    },
  );

  it("keeps the original target when atomic replacement fails", async () => {
    const transactionError = new Error("database unavailable");
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const oldAssistant: AnlgUIMessage = {
      id: "assistant-old",
      role: "assistant",
      parts: [{ type: "text", text: "Old answer" }],
    };
    const failedReplacement: AnlgUIMessage = {
      id: "assistant-failed",
      role: "assistant",
      parts: [{ type: "text", text: "Failed replacement" }],
    };
    const successfulReplacement: AnlgUIMessage = {
      id: "assistant-success",
      role: "assistant",
      parts: [{ type: "text", text: "Successful replacement" }],
    };
    mocks.messages = [userMessage, oldAssistant];
    mocks.getChatMessageGroupId.mockResolvedValue("group-1");
    mocks.replaceChatMessage
      .mockRejectedValueOnce(transactionError)
      .mockResolvedValueOnce(undefined);
    const sendTransport = connectChatRegenerateToTransport();

    const view = renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());
    const chat = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
        isError: boolean;
      }) => void;
    };
    chat.onFinish({
      isAbort: false,
      isError: false,
      message: failedReplacement,
      messages: [userMessage, failedReplacement],
    });
    await waitFor(() =>
      expect(consoleError).toHaveBeenCalledWith(
        "Failed to persist finished chat message",
        transactionError,
      ),
    );

    mocks.messages = [userMessage, failedReplacement];
    view.rerender(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {({ regenerate }) => (
          <button type="button" onClick={regenerate}>
            Regenerate
          </button>
        )}
      </ChatSession>,
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() => expect(sendTransport).toHaveBeenCalledTimes(2));
    chat.onFinish({
      isAbort: false,
      isError: false,
      message: successfulReplacement,
      messages: [userMessage, successfulReplacement],
    });

    await waitFor(() =>
      expect(mocks.replaceChatMessage).toHaveBeenLastCalledWith({
        message: expect.objectContaining({ id: successfulReplacement.id }),
        previousMessageId: oldAssistant.id,
      }),
    );
    consoleError.mockRestore();
  });

  it("holds CloudSync through the atomic regenerated answer replacement", async () => {
    let finishReplacement: (() => void) | undefined;
    const replacement = new Promise<void>((resolve) => {
      finishReplacement = resolve;
    });
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const oldAssistant: AnlgUIMessage = {
      id: "assistant-old",
      role: "assistant",
      parts: [{ type: "text", text: "Old answer" }],
    };
    const replacementMessage: AnlgUIMessage = {
      id: "assistant-new",
      role: "assistant",
      parts: [{ type: "text", text: "Replacement answer" }],
    };
    mocks.messages = [userMessage, oldAssistant];
    mocks.getChatMessageGroupId.mockResolvedValue("group-1");
    mocks.replaceChatMessage.mockReturnValueOnce(replacement);
    const sendTransport = connectChatRegenerateToTransport();

    renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());
    const nativeKey = startedNativeKey();
    const chat = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
        isError: boolean;
      }) => void;
    };
    chat.onFinish({
      isAbort: false,
      isError: false,
      message: replacementMessage,
      messages: [userMessage, replacementMessage],
    });

    await waitFor(() =>
      expect(mocks.replaceChatMessage).toHaveBeenCalledWith({
        message: expect.objectContaining({ id: replacementMessage.id }),
        previousMessageId: oldAssistant.id,
      }),
    );
    expect(mocks.endCloudsyncActivity).not.toHaveBeenCalledWith(
      "chat",
      nativeKey,
    );

    finishReplacement?.();
    await replacement;
    await waitFor(
      () =>
        expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith(
          "chat",
          nativeKey,
        ),
      { timeout: 1_500 },
    );
  });

  it("keeps the next prior assistant when cleanup cancels an active regeneration preflight", async () => {
    let finishFirstPersist: (() => void) | undefined;
    const firstPersist = new Promise<void>((resolve) => {
      finishFirstPersist = resolve;
    });
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const oldAssistant: AnlgUIMessage = {
      id: "assistant-old",
      role: "assistant",
      parts: [{ type: "text", text: "Old answer" }],
    };
    const firstReplacement: AnlgUIMessage = {
      id: "assistant-first",
      role: "assistant",
      parts: [{ type: "text", text: "First replacement" }],
    };
    mocks.messages = [userMessage, oldAssistant];
    mocks.getChatMessageGroupId.mockResolvedValue("group-1");
    mocks.replaceChatMessage.mockReturnValueOnce(firstPersist);
    const sendTransport = vi
      .fn()
      .mockResolvedValue(
        new ReadableStream({ start: (controller) => controller.close() }),
      );
    mocks.transport = {
      sendMessages: sendTransport,
      reconnectToStream: vi.fn().mockResolvedValue(null),
    };
    let requestCount = 0;
    mocks.chatRegenerate.mockImplementation(async () => {
      const init = mocks.chatInits[mocks.chatInits.length - 1] as {
        transport: {
          sendMessages: (options: {
            trigger: "regenerate-message";
            chatId: string;
            messageId: string | undefined;
            messages: AnlgUIMessage[];
            abortSignal: AbortSignal;
          }) => Promise<ReadableStream>;
        };
        onFinish: (params: {
          message: AnlgUIMessage;
          messages: AnlgUIMessage[];
          isAbort: boolean;
          isError: boolean;
        }) => void;
      };
      await init.transport.sendMessages({
        trigger: "regenerate-message",
        chatId: "session-1",
        messageId: undefined,
        messages: mocks.messages as AnlgUIMessage[],
        abortSignal: new AbortController().signal,
      });
      if (requestCount++ === 0) {
        init.onFinish({
          isAbort: false,
          isError: false,
          message: firstReplacement,
          messages: [userMessage, firstReplacement],
        });
      }
    });
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    const view = renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() =>
      expect(mocks.replaceChatMessage).toHaveBeenCalledWith({
        message: expect.objectContaining({ id: firstReplacement.id }),
        previousMessageId: oldAssistant.id,
      }),
    );

    mocks.messages = [userMessage, firstReplacement];
    view.rerender(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {({ regenerate }) => (
          <button type="button" onClick={regenerate}>
            Regenerate
          </button>
        )}
      </ChatSession>,
    );
    await Promise.resolve();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() =>
      expect(mocks.beginCloudsyncActivity).toHaveBeenCalledTimes(2),
    );
    expect(sendTransport).toHaveBeenCalledOnce();
    const nativeKeys = [startedNativeKey(0), startedNativeKey(1)];

    view.unmount();
    finishFirstPersist?.();
    await firstPersist;
    await waitFor(() => {
      for (const nativeKey of nativeKeys) {
        expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith(
          "chat",
          nativeKey,
        );
      }
    });

    expect(mocks.replaceChatMessage).not.toHaveBeenCalledWith(
      expect.objectContaining({
        previousMessageId: firstReplacement.id,
      }),
    );
    expect(sendTransport).toHaveBeenCalledOnce();
    consoleError.mockRestore();
  });

  it("ignores overlapping regeneration attempts", async () => {
    let finishTombstone: (() => void) | undefined;
    mocks.deleteChatMessage.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finishTombstone = resolve;
      }),
    );
    mocks.messages = [
      { id: "user-1", role: "user", parts: [{ type: "text", text: "Q" }] },
      {
        id: "assistant-1",
        role: "assistant",
        parts: [{ type: "text", text: "Old answer" }],
      },
    ];
    connectChatRegenerateToTransport();

    renderSession();
    const button = screen.getByRole("button", { name: "Regenerate" });
    fireEvent.click(button);
    fireEvent.click(button);

    expect(mocks.chatRegenerate).toHaveBeenCalledOnce();
    expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce();
    finishTombstone?.();
  });

  it("waits for regenerated assistant persistence before another retry", async () => {
    let finishAssistantPersist: (() => void) | undefined;
    const assistantPersist = new Promise<void>((resolve) => {
      finishAssistantPersist = resolve;
    });
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const oldAssistant: AnlgUIMessage = {
      id: "assistant-old",
      role: "assistant",
      parts: [{ type: "text", text: "Old answer" }],
    };
    const regeneratedAssistant: AnlgUIMessage = {
      id: "assistant-regenerated",
      role: "assistant",
      parts: [{ type: "text", text: "Regenerated answer" }],
    };
    mocks.messages = [userMessage, oldAssistant];
    mocks.getChatMessageGroupId.mockResolvedValue("group-1");
    mocks.replaceChatMessage.mockReturnValueOnce(assistantPersist);
    const sendTransport = vi
      .fn()
      .mockResolvedValue(
        new ReadableStream({ start: (controller) => controller.close() }),
      );
    mocks.transport = {
      sendMessages: sendTransport,
      reconnectToStream: vi.fn().mockResolvedValue(null),
    };
    let responseCount = 0;
    mocks.chatRegenerate.mockImplementation(async () => {
      const init = mocks.chatInits[mocks.chatInits.length - 1] as {
        transport: {
          sendMessages: (options: {
            trigger: "regenerate-message";
            chatId: string;
            messageId: string | undefined;
            messages: AnlgUIMessage[];
            abortSignal: AbortSignal;
          }) => Promise<ReadableStream>;
        };
        onFinish: (params: {
          message: AnlgUIMessage;
          messages: AnlgUIMessage[];
          isAbort: boolean;
        }) => void;
      };
      await init.transport.sendMessages({
        trigger: "regenerate-message",
        chatId: "session-1",
        messageId: undefined,
        messages: mocks.messages as AnlgUIMessage[],
        abortSignal: new AbortController().signal,
      });
      const assistant =
        responseCount++ === 0
          ? regeneratedAssistant
          : {
              id: "assistant-next",
              role: "assistant" as const,
              parts: [{ type: "text" as const, text: "Next answer" }],
            };
      init.onFinish({
        isAbort: false,
        message: assistant,
        messages: [userMessage, assistant],
      });
    });

    const view = renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() =>
      expect(mocks.replaceChatMessage).toHaveBeenCalledWith({
        message: expect.objectContaining({ id: regeneratedAssistant.id }),
        previousMessageId: oldAssistant.id,
      }),
    );

    mocks.messages = [userMessage, regeneratedAssistant];
    view.rerender(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {({ regenerate }) => (
          <button type="button" onClick={regenerate}>
            Regenerate
          </button>
        )}
      </ChatSession>,
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    expect(mocks.chatRegenerate).toHaveBeenCalledTimes(2);
    expect(mocks.replaceChatMessage).toHaveBeenCalledOnce();
    expect(sendTransport).toHaveBeenCalledOnce();

    finishAssistantPersist?.();
    await assistantPersist;

    await waitFor(() => expect(sendTransport).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(mocks.replaceChatMessage).toHaveBeenLastCalledWith({
        message: expect.objectContaining({ id: "assistant-next" }),
        previousMessageId: regeneratedAssistant.id,
      }),
    );
  });

  it("allows regeneration retries from the error state", async () => {
    mocks.status = "error";
    mocks.messages = [
      { id: "user-1", role: "user", parts: [{ type: "text", text: "Q" }] },
      {
        id: "assistant-1",
        role: "assistant",
        parts: [{ type: "text", text: "Failed answer" }],
      },
    ];
    connectChatRegenerateToTransport();

    renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() => expect(mocks.chatRegenerate).toHaveBeenCalledOnce());
    expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce();
  });

  it("recreates the SDK chat when transport becomes ready", () => {
    const initialTransport = {
      sendMessages: vi.fn(),
      reconnectToStream: vi.fn(),
    };
    const readyTransport = {
      sendMessages: vi.fn(),
      reconnectToStream: vi.fn(),
    };
    mocks.transport = initialTransport;

    const { rerender } = render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {() => null}
      </ChatSession>,
    );

    mocks.transport = readyTransport;
    rerender(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {() => null}
      </ChatSession>,
    );

    expect(mocks.chatInits).toHaveLength(2);
    expect((mocks.chatInits[0] as { transport: unknown }).transport).not.toBe(
      initialTransport,
    );
    expect((mocks.chatInits[1] as { transport: unknown }).transport).not.toBe(
      readyTransport,
    );
    expect((mocks.chatInits[0] as { transport: unknown }).transport).not.toBe(
      (mocks.chatInits[1] as { transport: unknown }).transport,
    );
  });

  it("syncs SDK messages when SQLite rows load later", async () => {
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const { rerender } = render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {() => null}
      </ChatSession>,
    );
    expect(mocks.chatInits).toHaveLength(1);

    mocks.persistedMessages = [persistedMessage(userMessage)];
    rerender(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {() => null}
      </ChatSession>,
    );

    await waitFor(() => {
      expect(mocks.chatSetMessages).toHaveBeenCalledWith([userMessage]);
    });
    expect(mocks.chatInits).toHaveLength(1);
  });

  it("keeps the SDK chat when first send creates a chat group", () => {
    const { rerender } = render(
      <ChatSession sessionId="session-1">{() => null}</ChatSession>,
    );

    rerender(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {() => null}
      </ChatSession>,
    );

    expect(mocks.chatInits).toHaveLength(1);
  });

  it("stops and drains the active lease when switching chat sessions", async () => {
    connectChatSendToTransport();
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const view = render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );

    captured.send!(userMessage);
    await waitFor(() =>
      expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce(),
    );
    const nativeKey = startedNativeKey();
    const oldChat = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    mocks.chatStop.mockImplementationOnce(() => {
      oldChat.onFinish({
        isAbort: true,
        message: { id: "assistant-1", role: "assistant", parts: [] },
        messages: [userMessage],
      });
    });

    view.rerender(
      <ChatSession chatGroupId="group-2" sessionId="session-2">
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );

    expect(mocks.chatStop).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith(
        "chat",
        nativeKey,
      ),
    );
    expect(mocks.chatInits).toHaveLength(2);
  });

  it("does not replace streaming SDK messages with stale SQLite rows", () => {
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    mocks.persistedMessages = [persistedMessage(userMessage)];
    mocks.messages = [
      userMessage,
      {
        id: "assistant-1",
        role: "assistant",
        parts: [{ type: "text", text: "Partial answer" }],
      },
    ];
    mocks.status = "streaming";

    render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {() => null}
      </ChatSession>,
    );

    expect(mocks.chatSetMessages).not.toHaveBeenCalled();
  });

  it("does not replace a finished response while its SQLite write is pending", async () => {
    let finishAssistantPersist: (() => void) | undefined;
    mocks.getChatMessageGroupId.mockResolvedValue("group-1");
    mocks.upsertChatMessage.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finishAssistantPersist = resolve;
      }),
    );
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const assistantMessage: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Finished answer" }],
    };
    mocks.persistedMessages = [persistedMessage(userMessage)];
    mocks.messages = [userMessage, assistantMessage];
    mocks.status = "streaming";

    const view = render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {() => null}
      </ChatSession>,
    );
    const chat = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    chat.onFinish({
      isAbort: false,
      message: assistantMessage,
      messages: [userMessage, assistantMessage],
    });

    await waitFor(() =>
      expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
        expect.objectContaining({ id: assistantMessage.id }),
      ),
    );
    mocks.status = "ready";
    view.rerender(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {() => null}
      </ChatSession>,
    );

    await Promise.resolve();
    expect(mocks.chatSetMessages).not.toHaveBeenCalled();

    finishAssistantPersist?.();
  });

  it("persists a first-send assistant response to the newly created group", async () => {
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    render(
      <ChatSession sessionId="session-1">
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );

    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    captured.send!(userMessage, { chatGroupId: "new-group" });
    const assistant: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Answer" }],
    };
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    onFinish.onFinish({
      isAbort: false,
      message: assistant,
      messages: [userMessage, assistant],
    });

    await waitFor(() =>
      expect(mocks.upsertChatMessage).toHaveBeenCalledTimes(2),
    );
    // The user row was missing at finish time, so the turn is repaired
    // before the assistant lands in the same group.
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "user-1",
        chatGroupId: "new-group",
      }),
    );
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "assistant-1",
        chatGroupId: "new-group",
        content: "Answer",
      }),
    );
  });

  it("falls back to the selected group when persisted routing lookup fails", async () => {
    const lookupError = new Error("database unavailable");
    mocks.getChatMessageGroupId.mockRejectedValue(lookupError);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const assistant: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Answer" }],
    };
    render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {() => null}
      </ChatSession>,
    );
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };

    onFinish.onFinish({
      isAbort: false,
      message: assistant,
      messages: [userMessage, assistant],
    });

    await waitFor(() =>
      expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
        expect.objectContaining({ id: "assistant-1", chatGroupId: "group-1" }),
      ),
    );
    expect(consoleError).toHaveBeenCalledWith(
      "Failed to resolve the persisted chat message group",
      lookupError,
    );
    consoleError.mockRestore();
  });

  it("persists overlapping assistant responses to their submitted groups", async () => {
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    render(
      <ChatSession chatGroupId="initial-group" sessionId="session-1">
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );

    const userOne: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "First question" }],
    };
    const userTwo: AnlgUIMessage = {
      id: "user-2",
      role: "user",
      parts: [{ type: "text", text: "Second question" }],
    };
    captured.send!(userOne, { chatGroupId: "group-1" });
    captured.send!(userTwo, { chatGroupId: "group-2" });

    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    const assistantOne: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "First answer" }],
    };
    const assistantTwo: AnlgUIMessage = {
      id: "assistant-2",
      role: "assistant",
      parts: [{ type: "text", text: "Second answer" }],
    };
    onFinish.onFinish({
      isAbort: false,
      message: assistantOne,
      messages: [userOne, assistantOne, userTwo],
    });
    onFinish.onFinish({
      isAbort: false,
      message: assistantTwo,
      messages: [userOne, assistantOne, userTwo, assistantTwo],
    });

    await waitFor(() =>
      expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
        expect.objectContaining({ id: "assistant-2", chatGroupId: "group-2" }),
      ),
    );
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({ id: "assistant-1", chatGroupId: "group-1" }),
    );
    // Unpersisted user turns are repaired alongside their assistant rows.
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({ id: "user-1", chatGroupId: "group-1" }),
    );
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({ id: "user-2", chatGroupId: "group-2" }),
    );
  });

  it("keeps CloudSync paused through persistence after unmount", async () => {
    let finishAssistantPersist: (() => void) | undefined;
    mocks.getChatMessageGroupId.mockResolvedValue("group-1");
    mocks.upsertChatMessage.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finishAssistantPersist = resolve;
      }),
    );
    connectChatSendToTransport();
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    const view = render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const assistant: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Answer" }],
    };

    captured.send!(userMessage);
    await waitFor(() => expect(mocks.chatSendMessage).toHaveBeenCalledOnce());
    mocks.endCloudsyncActivity.mockClear();
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    onFinish.onFinish({
      isAbort: false,
      message: assistant,
      messages: [userMessage, assistant],
    });

    await waitFor(() =>
      expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
        expect.objectContaining({ id: "assistant-1" }),
      ),
    );
    expect(mocks.endCloudsyncActivity).not.toHaveBeenCalled();

    view.unmount();
    await Promise.resolve();
    expect(mocks.endCloudsyncActivity).not.toHaveBeenCalled();

    finishAssistantPersist?.();
    await waitFor(
      () => expect(mocks.endCloudsyncActivity).toHaveBeenCalledOnce(),
      { timeout: 1_500 },
    );
    const nativeKey = startedNativeKey();
    expect(nativeKey).toMatch(/^user-1:/);
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith("chat", nativeKey);
  });

  it("persists a queued outgoing message after unmount wins lease acquisition", async () => {
    let acquireLease: (() => void) | undefined;
    let finishPersist: (() => void) | undefined;
    let finishTrackedWrite: (() => void) | undefined;
    mocks.beginCloudsyncActivity.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        acquireLease = resolve;
      }),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const sendTransport = connectChatSendToTransport();
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    const view = render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const beforeSend = vi.fn(
      (trackCompletion: (completion: Promise<unknown>) => void) => {
        trackCompletion(
          new Promise<void>((resolve) => {
            finishTrackedWrite = resolve;
          }),
        );
        return new Promise<void>((resolve) => {
          finishPersist = resolve;
        });
      },
    );

    captured.send!(userMessage, { beforeSend });
    await waitFor(() =>
      expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce(),
    );
    expect(beforeSend).not.toHaveBeenCalled();
    expect(sendTransport).not.toHaveBeenCalled();

    view.unmount();
    await Promise.resolve();
    expect(beforeSend).not.toHaveBeenCalled();
    expect(sendTransport).not.toHaveBeenCalled();

    acquireLease?.();
    await waitFor(() => expect(beforeSend).toHaveBeenCalledOnce());
    expect(mocks.endCloudsyncActivity).not.toHaveBeenCalled();
    expect(sendTransport).not.toHaveBeenCalled();

    finishPersist?.();
    await waitFor(() =>
      expect(consoleError).toHaveBeenCalledWith(
        "Failed to send chat message",
        expect.objectContaining({ name: "AbortError" }),
      ),
    );
    expect(mocks.endCloudsyncActivity).not.toHaveBeenCalled();

    finishTrackedWrite?.();
    await waitFor(() =>
      expect(mocks.endCloudsyncActivity).toHaveBeenCalledOnce(),
    );
    expect(sendTransport).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  it("keeps the existing assistant when regeneration unmounts during lease acquisition", async () => {
    let acquireLease: (() => void) | undefined;
    mocks.beginCloudsyncActivity.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        acquireLease = resolve;
      }),
    );
    mocks.messages = [
      {
        id: "user-1",
        role: "user",
        parts: [{ type: "text", text: "Question" }],
      },
      {
        id: "assistant-1",
        role: "assistant",
        parts: [{ type: "text", text: "Existing answer" }],
      },
    ];
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const sendTransport = connectChatRegenerateToTransport();
    const view = renderSession();

    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
    await waitFor(() =>
      expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce(),
    );
    expect(mocks.deleteChatMessage).not.toHaveBeenCalled();

    view.unmount();
    acquireLease?.();
    await waitFor(() =>
      expect(mocks.endCloudsyncActivity).toHaveBeenCalledOnce(),
    );

    expect(mocks.deleteChatMessage).not.toHaveBeenCalled();
    expect(sendTransport).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  it("releases the chat lease after an aborted response", async () => {
    connectChatSendToTransport();
    const captured: {
      send?: ChatSessionRenderProps["sendMessage"];
      stop?: ChatSessionRenderProps["stop"];
    } = {};
    render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {(props) => {
          captured.send = props.sendMessage;
          captured.stop = props.stop;
          return null;
        }}
      </ChatSession>,
    );
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };

    captured.send!(userMessage);
    await waitFor(() => expect(mocks.chatSendMessage).toHaveBeenCalledOnce());
    mocks.endCloudsyncActivity.mockClear();
    captured.stop!();
    expect(mocks.chatStop).toHaveBeenCalledOnce();

    const assistant: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [],
    };
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    onFinish.onFinish({
      isAbort: true,
      message: assistant,
      messages: [userMessage, assistant],
    });

    expect(mocks.upsertChatMessage).not.toHaveBeenCalled();
    await waitFor(
      () => expect(mocks.endCloudsyncActivity).toHaveBeenCalledOnce(),
      { timeout: 1_500 },
    );
  });

  it("releases after durable writes flush even when SDK stop never settles", async () => {
    let finishWriteFlush: (() => void) | undefined;
    mocks.chatStop.mockReturnValueOnce(new Promise<void>(() => {}));
    mocks.flushDatabaseWritesByPrefix.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finishWriteFlush = resolve;
      }),
    );
    connectChatSendToTransport();
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    const view = render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );

    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    captured.send!(userMessage);
    await waitFor(() =>
      expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce(),
    );
    const nativeKey = startedNativeKey();
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    mocks.endCloudsyncActivity.mockClear();

    view.unmount();
    await waitFor(() =>
      expect(mocks.flushDatabaseWritesByPrefix).toHaveBeenCalledWith(["chat:"]),
    );

    expect(mocks.endCloudsyncActivity).not.toHaveBeenCalled();
    expect(mocks.chatStop).toHaveBeenCalledOnce();

    finishWriteFlush?.();
    await waitFor(() =>
      expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith(
        "chat",
        nativeKey,
      ),
    );
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledTimes(1);

    mocks.upsertChatMessage.mockClear();
    onFinish.onFinish({
      isAbort: false,
      message: {
        id: "assistant-late",
        role: "assistant",
        parts: [{ type: "text", text: "Late answer" }],
      },
      messages: [
        userMessage,
        {
          id: "assistant-late",
          role: "assistant",
          parts: [{ type: "text", text: "Late answer" }],
        },
      ],
    });
    await Promise.resolve();

    expect(mocks.upsertChatMessage).not.toHaveBeenCalled();
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledTimes(1);
  });

  it("can start chat after StrictMode replays mount cleanup", async () => {
    const sendTransport = connectChatSendToTransport();
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    render(
      <StrictMode>
        <ChatSession chatGroupId="group-1" sessionId="session-1">
          {(props) => {
            captured.send = props.sendMessage;
            return null;
          }}
        </ChatSession>
      </StrictMode>,
    );
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };

    captured.send!(userMessage);

    await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());
    expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce();

    const assistant: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [],
    };
    const onFinish = mocks.chatInits[mocks.chatInits.length - 1] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    onFinish.onFinish({
      isAbort: true,
      message: assistant,
      messages: [userMessage, assistant],
    });
    await waitFor(() =>
      expect(mocks.endCloudsyncActivity).toHaveBeenCalledOnce(),
    );
  });

  it("waits for the chat lease and durable preflight before transport", async () => {
    let acquireLease: (() => void) | undefined;
    let finishPreflight: (() => void) | undefined;
    mocks.beginCloudsyncActivity.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        acquireLease = resolve;
      }),
    );
    const beforeSend = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finishPreflight = resolve;
        }),
    );
    const sendTransport = connectChatSendToTransport();
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    const view = render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };

    captured.send!(userMessage, { beforeSend });

    expect(mocks.chatSendMessage).toHaveBeenCalledWith(userMessage);
    const nativeKey = startedNativeKey();
    expect(nativeKey).toMatch(/^user-1:/);
    expect(mocks.beginCloudsyncActivity).toHaveBeenCalledWith(
      "chat",
      nativeKey,
    );
    expect(beforeSend).not.toHaveBeenCalled();
    expect(sendTransport).not.toHaveBeenCalled();

    acquireLease?.();
    await waitFor(() => expect(beforeSend).toHaveBeenCalledOnce());
    expect(sendTransport).not.toHaveBeenCalled();

    finishPreflight?.();
    await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());

    const assistant: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [],
    };
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    onFinish.onFinish({
      isAbort: true,
      message: assistant,
      messages: [userMessage, assistant],
    });
    await waitFor(() =>
      expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith(
        "chat",
        nativeKey,
      ),
    );
    view.unmount();
  });

  it("repairs a failed preflight before the same message can retry", async () => {
    const acquisitionError = new Error("cloudsync busy");
    mocks.beginCloudsyncActivity
      .mockRejectedValueOnce(acquisitionError)
      .mockResolvedValueOnce(undefined);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const sendTransport = connectChatSendToTransport();
    const captured: { send?: ChatSessionRenderProps["sendMessage"] } = {};
    render(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {(props) => {
          captured.send = props.sendMessage;
          return null;
        }}
      </ChatSession>,
    );
    const userMessage: AnlgUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const failedPreflight = vi.fn().mockResolvedValue(undefined);
    const retryPreflight = vi.fn().mockResolvedValue(undefined);

    captured.send!(userMessage, { beforeSend: failedPreflight });
    await waitFor(() =>
      expect(consoleError).toHaveBeenCalledWith(
        "Failed to send chat message",
        acquisitionError,
      ),
    );

    expect(failedPreflight).toHaveBeenCalledOnce();
    expect(sendTransport).not.toHaveBeenCalled();
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith(
      "chat",
      startedNativeKey(0),
    );

    captured.send!(userMessage, { beforeSend: retryPreflight });
    await waitFor(() => expect(retryPreflight).toHaveBeenCalledOnce());
    expect(failedPreflight).toHaveBeenCalledOnce();
    expect(sendTransport).toHaveBeenCalledOnce();

    const assistant: AnlgUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [],
    };
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: AnlgUIMessage;
        messages: AnlgUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    onFinish.onFinish({
      isAbort: true,
      message: assistant,
      messages: [userMessage, assistant],
    });
    await waitFor(
      () =>
        expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith(
          "chat",
          startedNativeKey(2),
        ),
      { timeout: 1_500 },
    );
    consoleError.mockRestore();
  });
});
