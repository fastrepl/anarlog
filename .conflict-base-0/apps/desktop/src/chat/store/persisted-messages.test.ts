import { describe, expect, test } from "vitest";

import {
  buildPersistedChatMessage,
  getVisibleChatMessages,
  normalizeChatMessageStatus,
  rowToPersistedChatMessage,
  shouldHidePersistedMessage,
  shouldPersistFinishedMessage,
  type ChatMessageSqlRow,
} from "./persisted-messages";

import type { HyprUIMessage } from "~/chat/types";

function chatMessageRow(
  overrides: Partial<ChatMessageSqlRow> = {},
): ChatMessageSqlRow {
  return {
    id: "assistant-1",
    owner_user_id: "user-1",
    created_at: "2024-01-01T00:00:01.000Z",
    chat_group_id: "group-1",
    role: "assistant",
    content: "Hello",
    metadata_json: '{"createdAt":1704067201000}',
    parts_json: '[{"type":"text","text":"Hello"}]',
    status: "ready",
    ...overrides,
  };
}

describe("persisted chat messages", () => {
  test("defaults unknown status to ready", () => {
    expect(normalizeChatMessageStatus(undefined)).toBe("ready");
    expect(normalizeChatMessageStatus("unexpected")).toBe("ready");
  });

  test("builds a canonical SQLite record from a UI message", () => {
    const message: HyprUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Hello" }],
      metadata: { createdAt: Date.parse("2024-01-01T00:00:01Z") },
    };

    expect(
      buildPersistedChatMessage({
        message,
        chatGroupId: "group-1",
        ownerUserId: "user-1",
        status: "streaming",
      }),
    ).toEqual({
      id: "assistant-1",
      ownerUserId: "user-1",
      createdAt: "2024-01-01T00:00:01.000Z",
      chatGroupId: "group-1",
      role: "assistant",
      content: "Hello",
      metadataJson: '{"createdAt":1704067201000}',
      partsJson: '[{"type":"text","text":"Hello"}]',
      status: "streaming",
    });
  });

  test("parses canonical SQLite rows back into UI messages", () => {
    const parsed = rowToPersistedChatMessage(chatMessageRow());

    expect(parsed.status).toBe("ready");
    expect(parsed.message).toEqual({
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Hello" }],
      metadata: { createdAt: 1704067201000 },
    });
  });

  test("hides empty assistant messages regardless of status", () => {
    const empty = rowToPersistedChatMessage(
      chatMessageRow({ content: "", parts_json: "[]", status: "streaming" }),
    );
    const readyEmpty = rowToPersistedChatMessage(
      chatMessageRow({
        id: "assistant-ready",
        content: "",
        parts_json: "[]",
      }),
    );
    const visible = rowToPersistedChatMessage(
      chatMessageRow({ status: "streaming" }),
    );

    expect(shouldHidePersistedMessage(empty)).toBe(true);
    expect(shouldHidePersistedMessage(readyEmpty)).toBe(true);
    expect(shouldHidePersistedMessage(visible)).toBe(false);
    expect(getVisibleChatMessages([empty, visible])).toEqual([visible.message]);
  });

  test("does not persist empty finished assistant messages", () => {
    expect(
      shouldPersistFinishedMessage({
        id: "assistant-empty",
        role: "assistant",
        parts: [],
      }),
    ).toBe(false);

    expect(
      shouldPersistFinishedMessage({
        id: "assistant-text",
        role: "assistant",
        parts: [{ type: "text", text: "Done" }],
      }),
    ).toBe(true);
  });
});
