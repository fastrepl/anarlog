import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { PersistedChatMessage } from "~/chat/store/persisted-messages";

const mocks = vi.hoisted(() => ({
  chatRegenerate: vi.fn(),
  chatSendMessage: vi.fn(),
  chatSetMessages: vi.fn(),
  chatStop: vi.fn(),
  chatInits: [] as unknown[],
  deleteChatMessage: vi.fn().mockResolvedValue(undefined),
  deleteChatMessagesExcept: vi.fn().mockResolvedValue(undefined),
  getChatMessageGroupId: vi.fn().mockResolvedValue(null),
  messages: [] as unknown[],
  persistedMessages: [] as PersistedChatMessage[],
  status: "ready",
  store: {} as unknown,
  transport: {} as unknown,
  upsertChatMessage: vi.fn().mockResolvedValue(undefined),
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
  render(
    <ChatSession chatGroupId="group-1" sessionId="session-1">
      {({ regenerate }) => (
        <button type="button" onClick={regenerate}>
          Regenerate
        </button>
      )}
    </ChatSession>,
  );
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
    mocks.deleteChatMessage.mockResolvedValue(undefined);
    mocks.deleteChatMessagesExcept.mockResolvedValue(undefined);
    mocks.getChatMessageGroupId.mockResolvedValue(null);
    mocks.upsertChatMessage.mockResolvedValue(undefined);
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

    renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() => expect(mocks.chatRegenerate).toHaveBeenCalledOnce());
    expect(mocks.deleteChatMessage).toHaveBeenCalledWith(
      "group-1",
      "assistant-empty",
    );
    expect(mocks.deleteChatMessage).not.toHaveBeenCalledWith(
      "group-1",
      "assistant-previous",
    );
  });

  it("tombstones the last assistant before regeneration", async () => {
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

    renderSession();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    await waitFor(() => expect(mocks.chatRegenerate).toHaveBeenCalledOnce());
    expect(mocks.deleteChatMessage).toHaveBeenCalledWith(
      "group-1",
      "assistant-current",
    );
    expect(mocks.deleteChatMessage.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.chatRegenerate.mock.invocationCallOrder[0],
    );
  });

  it("recreates the SDK chat when transport becomes ready", () => {
    const initialTransport = {};
    const readyTransport = {};
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
    expect((mocks.chatInits[0] as { transport: unknown }).transport).toBe(
      initialTransport,
    );
    expect((mocks.chatInits[1] as { transport: unknown }).transport).toBe(
      readyTransport,
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
});
