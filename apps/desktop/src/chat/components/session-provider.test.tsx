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
  getChatMessageGroupId: vi.fn().mockResolvedValue(null),
  messages: [] as unknown[],
  persistedMessages: [] as PersistedChatMessage[],
  status: "ready",
  store: {} as unknown,
  transport: {} as unknown,
  upsertChatMessage: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@hypr/plugin-db", () => ({
  beginCloudsyncActivity: mocks.beginCloudsyncActivity,
  endCloudsyncActivity: mocks.endCloudsyncActivity,
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
import type { HyprUIMessage } from "~/chat/types";

function persistedMessage(
  message: HyprUIMessage,
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
  mocks.chatSendMessage.mockImplementation((message: HyprUIMessage) => {
    const init = mocks.chatInits[mocks.chatInits.length - 1] as {
      transport: {
        sendMessages: (options: {
          trigger: "submit-message";
          chatId: string;
          messageId: undefined;
          messages: HyprUIMessage[];
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
          messages: HyprUIMessage[];
          abortSignal: AbortSignal;
        }) => Promise<ReadableStream>;
      };
    };
    return init.transport.sendMessages({
      trigger: "regenerate-message",
      chatId: "session-1",
      messageId: undefined,
      messages: mocks.messages as HyprUIMessage[],
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
    mocks.getChatMessageGroupId.mockReset().mockResolvedValue(null);
    mocks.upsertChatMessage.mockReset().mockResolvedValue(undefined);
  });

  it("does not delete the previous assistant when retrying an unpersisted empty assistant", async () => {
    const previousAssistant: HyprUIMessage = {
      id: "assistant-previous",
      role: "assistant",
      parts: [{ type: "text", text: "Previous answer" }],
    };
    mocks.persistedMessages = [persistedMessage(previousAssistant)];
    mocks.messages = [
      { id: "user-1", role: "user", parts: [{ type: "text", text: "Q1" }] },
      previousAssistant,
      { id: "user-2", role: "user", parts: [{ type: "text", text: "Q2" }] },
      { id: "assistant-empty", role: "assistant", parts: [] },
    ];
    connectChatRegenerateToTransport();

    renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() =>
      expect(mocks.deleteChatMessage).toHaveBeenCalledWith(
        "group-1",
        "assistant-empty",
      ),
    );
    expect(mocks.deleteChatMessage).not.toHaveBeenCalledWith(
      "group-1",
      "assistant-previous",
    );
  });

  it("pauses sync before tombstoning and starting regeneration", async () => {
    let finishTombstone: (() => void) | undefined;
    mocks.deleteChatMessage.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finishTombstone = resolve;
      }),
    );
    const assistant: HyprUIMessage = {
      id: "assistant-current",
      role: "assistant",
      parts: [{ type: "text", text: "Current answer" }],
    };
    mocks.messages = [
      { id: "user-1", role: "user", parts: [{ type: "text", text: "Q" }] },
      assistant,
      {
        id: "user-2",
        role: "user",
        parts: [{ type: "text", text: "Follow-up" }],
      },
    ];
    const sendTransport = connectChatRegenerateToTransport();

    renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() => expect(mocks.chatRegenerate).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(mocks.beginCloudsyncActivity).toHaveBeenCalled(),
    );
    expect(mocks.deleteChatMessage).toHaveBeenCalledWith(
      "group-1",
      "assistant-current",
    );
    expect(
      mocks.beginCloudsyncActivity.mock.invocationCallOrder[0],
    ).toBeLessThan(mocks.deleteChatMessage.mock.invocationCallOrder[0]);
    expect(sendTransport).not.toHaveBeenCalled();

    finishTombstone?.();
    await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());
  });

  it("does not start a duplicate response when the tombstone fails", async () => {
    let finishRetryTombstone: (() => void) | undefined;
    const tombstoneError = new Error("database unavailable");
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.deleteChatMessage
      .mockRejectedValueOnce(tombstoneError)
      .mockReturnValueOnce(
        new Promise<void>((resolve) => {
          finishRetryTombstone = resolve;
        }),
      );
    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Q" }],
    };
    mocks.messages = [
      userMessage,
      {
        id: "assistant-1",
        role: "assistant",
        parts: [{ type: "text", text: "Old answer" }],
      },
    ];
    const sendTransport = connectChatRegenerateToTransport();

    const view = renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() =>
      expect(consoleError).toHaveBeenCalledWith(
        "Failed to remove regenerated chat message",
        tombstoneError,
      ),
    );
    expect(sendTransport).not.toHaveBeenCalled();

    await waitFor(() =>
      expect(consoleError).toHaveBeenCalledWith(
        "Failed to regenerate chat message",
        tombstoneError,
      ),
    );
    mocks.messages = [userMessage];
    mocks.status = "error";
    view.rerender(
      <ChatSession chatGroupId="group-1" sessionId="session-1">
        {({ regenerate }) => (
          <button type="button" onClick={regenerate}>
            Regenerate
          </button>
        )}
      </ChatSession>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() =>
      expect(mocks.deleteChatMessage).toHaveBeenCalledTimes(2),
    );
    expect(mocks.deleteChatMessage).toHaveBeenLastCalledWith(
      "group-1",
      "assistant-1",
    );
    expect(
      mocks.beginCloudsyncActivity.mock.invocationCallOrder[1],
    ).toBeLessThan(mocks.deleteChatMessage.mock.invocationCallOrder[1]);
    expect(sendTransport).not.toHaveBeenCalled();

    finishRetryTombstone?.();
    await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());
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
    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const oldAssistant: HyprUIMessage = {
      id: "assistant-old",
      role: "assistant",
      parts: [{ type: "text", text: "Old answer" }],
    };
    const regeneratedAssistant: HyprUIMessage = {
      id: "assistant-regenerated",
      role: "assistant",
      parts: [{ type: "text", text: "Regenerated answer" }],
    };
    mocks.messages = [userMessage, oldAssistant];
    mocks.getChatMessageGroupId.mockResolvedValue("group-1");
    mocks.upsertChatMessage.mockReturnValueOnce(assistantPersist);
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
            messages: HyprUIMessage[];
            abortSignal: AbortSignal;
          }) => Promise<ReadableStream>;
        };
        onFinish: (params: {
          message: HyprUIMessage;
          messages: HyprUIMessage[];
          isAbort: boolean;
        }) => void;
      };
      await init.transport.sendMessages({
        trigger: "regenerate-message",
        chatId: "session-1",
        messageId: undefined,
        messages: mocks.messages as HyprUIMessage[],
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
      expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
        expect.objectContaining({ id: regeneratedAssistant.id }),
      ),
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
    expect(mocks.deleteChatMessage).not.toHaveBeenCalledWith(
      "group-1",
      regeneratedAssistant.id,
    );
    expect(sendTransport).toHaveBeenCalledOnce();

    finishAssistantPersist?.();
    await assistantPersist;

    await waitFor(() =>
      expect(mocks.deleteChatMessage).toHaveBeenCalledWith(
        "group-1",
        regeneratedAssistant.id,
      ),
    );
    await waitFor(() => expect(sendTransport).toHaveBeenCalledTimes(2));
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
    const userMessage: HyprUIMessage = {
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
    const userMessage: HyprUIMessage = {
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
        message: HyprUIMessage;
        messages: HyprUIMessage[];
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
    const userMessage: HyprUIMessage = {
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
    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const assistantMessage: HyprUIMessage = {
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
        message: HyprUIMessage;
        messages: HyprUIMessage[];
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

    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    captured.send!(userMessage, { chatGroupId: "new-group" });
    const assistant: HyprUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Answer" }],
    };
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: HyprUIMessage;
        messages: HyprUIMessage[];
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
    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const assistant: HyprUIMessage = {
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
        message: HyprUIMessage;
        messages: HyprUIMessage[];
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

    const userOne: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "First question" }],
    };
    const userTwo: HyprUIMessage = {
      id: "user-2",
      role: "user",
      parts: [{ type: "text", text: "Second question" }],
    };
    captured.send!(userOne, { chatGroupId: "group-1" });
    captured.send!(userTwo, { chatGroupId: "group-2" });

    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: HyprUIMessage;
        messages: HyprUIMessage[];
        isAbort: boolean;
      }) => void;
    };
    const assistantOne: HyprUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "First answer" }],
    };
    const assistantTwo: HyprUIMessage = {
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
    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    const assistant: HyprUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Answer" }],
    };

    captured.send!(userMessage);
    await waitFor(() => expect(mocks.chatSendMessage).toHaveBeenCalledOnce());
    mocks.endCloudsyncActivity.mockClear();
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: HyprUIMessage;
        messages: HyprUIMessage[];
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
    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };

    captured.send!(userMessage);
    await waitFor(() => expect(mocks.chatSendMessage).toHaveBeenCalledOnce());
    mocks.endCloudsyncActivity.mockClear();
    captured.stop!();
    expect(mocks.chatStop).toHaveBeenCalledOnce();

    const assistant: HyprUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [],
    };
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: HyprUIMessage;
        messages: HyprUIMessage[];
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

  it("keeps an unmounted chat paused until SDK cleanup finishes", async () => {
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

    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };
    captured.send!(userMessage);
    await waitFor(() =>
      expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce(),
    );
    const nativeKey = startedNativeKey();
    mocks.endCloudsyncActivity.mockClear();

    view.unmount();
    await Promise.resolve();

    expect(mocks.endCloudsyncActivity).not.toHaveBeenCalled();
    expect(mocks.chatStop).toHaveBeenCalledOnce();

    const assistant: HyprUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [],
    };
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: HyprUIMessage;
        messages: HyprUIMessage[];
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
    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };

    captured.send!(userMessage);

    await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());
    expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce();

    const assistant: HyprUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [],
    };
    const onFinish = mocks.chatInits[mocks.chatInits.length - 1] as {
      onFinish: (params: {
        message: HyprUIMessage;
        messages: HyprUIMessage[];
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

  it("updates SDK state immediately while transport waits for the chat lease", async () => {
    let acquireLease: (() => void) | undefined;
    mocks.beginCloudsyncActivity.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        acquireLease = resolve;
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
    const userMessage: HyprUIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "Question" }],
    };

    captured.send!(userMessage);

    expect(mocks.chatSendMessage).toHaveBeenCalledWith(userMessage);
    const nativeKey = startedNativeKey();
    expect(nativeKey).toMatch(/^user-1:/);
    expect(mocks.beginCloudsyncActivity).toHaveBeenCalledWith(
      "chat",
      nativeKey,
    );
    expect(sendTransport).not.toHaveBeenCalled();

    acquireLease?.();
    await waitFor(() => expect(sendTransport).toHaveBeenCalledOnce());

    const assistant: HyprUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [],
    };
    const onFinish = mocks.chatInits[0] as {
      onFinish: (params: {
        message: HyprUIMessage;
        messages: HyprUIMessage[];
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
});
