import { describe, expect, test } from "vitest";

import {
  buildPersistedChatMessageRow,
  normalizeChatMessageStatus,
  rowToPersistedChatMessage,
  shouldHidePersistedMessage,
} from "./persisted-messages";

import type { HyprUIMessage } from "~/chat/types";

describe("persisted chat messages", () => {
  test("defaults unknown status to ready", () => {
    expect(normalizeChatMessageStatus(undefined)).toBe("ready");
    expect(normalizeChatMessageStatus("unexpected")).toBe("ready");
  });

  test("builds a persisted row from a UI message", () => {
    const message: HyprUIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Hello" }],
      metadata: { createdAt: Date.parse("2024-01-01T00:00:01Z") },
    };

    expect(
      buildPersistedChatMessageRow({
        message,
        chatGroupId: "group-1",
        userId: "user-1",
        status: "streaming",
      }),
    ).toEqual({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:01.000Z",
      chat_group_id: "group-1",
      role: "assistant",
      content: "Hello",
      metadata: '{"createdAt":1704067201000}',
      parts: '[{"type":"text","text":"Hello"}]',
      status: "streaming",
    });
  });

  test("parses persisted rows back into UI messages", () => {
    const parsed = rowToPersistedChatMessage("assistant-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:01.000Z",
      chat_group_id: "group-1",
      role: "assistant",
      content: "Hello",
      metadata: '{"createdAt":1704067201000}',
      parts: '[{"type":"text","text":"Hello"}]',
      status: "ready",
    });

    expect(parsed.status).toBe("ready");
    expect(parsed.message).toEqual({
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Hello" }],
      metadata: { createdAt: 1704067201000 },
    });
  });

  test("hides only empty streaming assistant messages", () => {
    expect(
      shouldHidePersistedMessage(
        rowToPersistedChatMessage("assistant-1", {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:01.000Z",
          chat_group_id: "group-1",
          role: "assistant",
          content: "",
          metadata: "{}",
          parts: "[]",
          status: "streaming",
        }),
      ),
    ).toBe(true);

    expect(
      shouldHidePersistedMessage(
        rowToPersistedChatMessage("assistant-2", {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:02.000Z",
          chat_group_id: "group-1",
          role: "assistant",
          content: "Visible",
          metadata: "{}",
          parts: '[{"type":"text","text":"Visible"}]',
          status: "streaming",
        }),
      ),
    ).toBe(false);
  });
});
