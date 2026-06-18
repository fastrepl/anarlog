import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatRegenerate: vi.fn(),
  chatSendMessage: vi.fn(),
  chatSetMessages: vi.fn(),
  chatStop: vi.fn(),
  messages: [] as unknown[],
  store: null as unknown,
}));

vi.mock("@ai-sdk/react", () => ({
  useChat: () => ({
    messages: mocks.messages,
    sendMessage: mocks.chatSendMessage,
    regenerate: mocks.chatRegenerate,
    stop: mocks.chatStop,
    status: "ready",
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

vi.mock("~/chat/transport/use-transport", () => ({
  useTransport: () => ({
    transport: {},
    isSystemPromptReady: true,
  }),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useStore: () => mocks.store,
    useValues: () => ({ user_id: "user-1" }),
  },
}));

import { ChatSession } from "./session-provider";

import { buildPersistedChatMessageRow } from "~/chat/store/persisted-messages";
import type { HyprUIMessage } from "~/chat/types";

type FakeStore = ReturnType<typeof createStore>;

function createStore(rows: Record<string, Record<string, unknown>>) {
  const table = new Map(Object.entries(rows));

  return {
    delRow: vi.fn((tableName: string, rowId: string) => {
      if (tableName === "chat_messages") {
        table.delete(rowId);
      }
    }),
    forEachRow: vi.fn(
      (
        tableName: string,
        callback: (rowId: string, forEachCell: () => void) => void,
      ) => {
        if (tableName !== "chat_messages") {
          return;
        }
        table.forEach((_row, rowId) => callback(rowId, () => {}));
      },
    ),
    getRow: vi.fn((tableName: string, rowId: string) => {
      if (tableName !== "chat_messages") {
        return undefined;
      }
      return table.get(rowId);
    }),
    setRow: vi.fn(
      (tableName: string, rowId: string, row: Record<string, unknown>) => {
        if (tableName === "chat_messages") {
          table.set(rowId, row);
        }
      },
    ),
    transaction: vi.fn((callback: () => void) => callback()),
  };
}

function persistedAssistantRow(message: HyprUIMessage) {
  return buildPersistedChatMessageRow({
    message,
    chatGroupId: "group-1",
    userId: "user-1",
    status: "ready",
  }) as unknown as Record<string, unknown>;
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
    mocks.chatRegenerate.mockClear();
    mocks.chatSendMessage.mockClear();
    mocks.chatSetMessages.mockClear();
    mocks.chatStop.mockClear();
    mocks.messages = [];
    mocks.store = createStore({});
  });

  it("does not delete the previous persisted assistant when retrying an unpersisted empty assistant", () => {
    const previousAssistant: HyprUIMessage = {
      id: "assistant-previous",
      role: "assistant",
      parts: [{ type: "text", text: "Previous answer" }],
      metadata: { createdAt: Date.parse("2024-01-01T00:00:01Z") },
    };
    const store = createStore({
      "assistant-previous": persistedAssistantRow(previousAssistant),
    });
    mocks.store = store;
    mocks.messages = [
      {
        id: "user-1",
        role: "user",
        parts: [{ type: "text", text: "First question" }],
      },
      previousAssistant,
      {
        id: "user-2",
        role: "user",
        parts: [{ type: "text", text: "Retry question" }],
      },
      {
        id: "assistant-empty",
        role: "assistant",
        parts: [],
      },
    ];

    renderSession();

    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    expect(store.delRow).toHaveBeenCalledWith(
      "chat_messages",
      "assistant-empty",
    );
    expect(
      (store as FakeStore).getRow("chat_messages", "assistant-previous"),
    ).toBeDefined();
    expect(mocks.chatRegenerate).toHaveBeenCalledTimes(1);
  });

  it("deletes the persisted row for the in-memory assistant being regenerated", () => {
    const assistant: HyprUIMessage = {
      id: "assistant-current",
      role: "assistant",
      parts: [{ type: "text", text: "Current answer" }],
      metadata: { createdAt: Date.parse("2024-01-01T00:00:01Z") },
    };
    const store = createStore({
      "assistant-current": persistedAssistantRow(assistant),
    });
    mocks.store = store;
    mocks.messages = [
      {
        id: "user-1",
        role: "user",
        parts: [{ type: "text", text: "Question" }],
      },
      assistant,
    ];

    renderSession();

    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    expect(store.delRow).toHaveBeenCalledWith(
      "chat_messages",
      "assistant-current",
    );
    expect(store.getRow("chat_messages", "assistant-current")).toBeUndefined();
    expect(mocks.chatRegenerate).toHaveBeenCalledTimes(1);
  });

  it("deletes the last assistant row when a trailing user message is present", () => {
    const assistant: HyprUIMessage = {
      id: "assistant-current",
      role: "assistant",
      parts: [{ type: "text", text: "Current answer" }],
      metadata: { createdAt: Date.parse("2024-01-01T00:00:01Z") },
    };
    const store = createStore({
      "assistant-current": persistedAssistantRow(assistant),
    });
    mocks.store = store;
    mocks.messages = [
      {
        id: "user-1",
        role: "user",
        parts: [{ type: "text", text: "Question" }],
      },
      assistant,
      {
        id: "user-2",
        role: "user",
        parts: [{ type: "text", text: "Follow-up" }],
      },
    ];

    renderSession();

    fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));

    expect(store.delRow).toHaveBeenCalledWith(
      "chat_messages",
      "assistant-current",
    );
    expect(store.getRow("chat_messages", "assistant-current")).toBeUndefined();
    expect(mocks.chatRegenerate).toHaveBeenCalledTimes(1);
  });
});
