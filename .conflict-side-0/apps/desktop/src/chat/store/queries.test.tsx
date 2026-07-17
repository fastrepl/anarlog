import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn().mockResolvedValue([1]),
  rows: [] as Array<Record<string, unknown>>,
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: (options: {
    enabled?: boolean;
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  }) => ({
    data: options.enabled === false ? undefined : options.mapRows(mocks.rows),
  }),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, write: () => Promise<unknown>) =>
    write(),
}));

import type { ChatMessageRecord } from "./persisted-messages";
import {
  createChatGroupWithMessage,
  deleteChatMessage,
  deleteChatMessagesExcept,
  getChatMessageGroupId,
  setChatGroupTitleIfCurrent,
  upsertChatMessage,
  useChatGroup,
  usePersistedChatMessages,
  useRecentChatGroups,
} from "./queries";

const message: ChatMessageRecord = {
  id: "message-1",
  ownerUserId: "user-1",
  createdAt: "2026-07-10T10:00:01.000Z",
  chatGroupId: "group-1",
  role: "user",
  content: "Hello",
  metadataJson: "{}",
  partsJson: '[{"type":"text","text":"Hello"}]',
  status: "ready",
};

describe("chat SQLite queries", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.rows = [];
    mocks.execute.mockResolvedValue([]);
    mocks.executeTransaction.mockResolvedValue([1]);
  });

  it("maps recent active chat groups", () => {
    mocks.rows = [
      {
        id: "group-1",
        owner_user_id: "user-1",
        title: "Planning",
        created_at: "2026-07-10T10:00:00.000Z",
        updated_at: "2026-07-10T10:01:00.000Z",
      },
    ];

    const recent = renderHook(() => useRecentChatGroups()).result.current;
    const selected = renderHook(() => useChatGroup("group-1")).result.current;

    expect(recent).toEqual([
      {
        id: "group-1",
        ownerUserId: "user-1",
        title: "Planning",
        createdAt: "2026-07-10T10:00:00.000Z",
        updatedAt: "2026-07-10T10:01:00.000Z",
      },
    ]);
    expect(selected?.id).toBe("group-1");
  });

  it("maps chat messages in their durable order", () => {
    mocks.rows = [
      {
        id: "message-1",
        owner_user_id: "user-1",
        created_at: "2026-07-10T10:00:01.000Z",
        chat_group_id: "group-1",
        role: "user",
        content: "Hello",
        metadata_json: "{}",
        parts_json: '[{"type":"text","text":"Hello"}]',
        status: "ready",
      },
    ];

    const { result } = renderHook(() => usePersistedChatMessages("group-1"));

    expect(result.current).toHaveLength(1);
    expect(result.current[0].message.parts).toEqual([
      { type: "text", text: "Hello" },
    ]);
  });

  it("commits a new group and its first message atomically", async () => {
    await createChatGroupWithMessage({
      groupId: "group-1",
      ownerUserId: "user-1",
      title: "Hello",
      createdAt: "2026-07-10T10:00:00.000Z",
      message,
    });

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("INSERT INTO chat_groups");
    expect(statements[0].sql).toContain("deleted_at = NULL");
    expect(statements[1].sql).toContain("INSERT INTO chat_messages");
    expect(statements[1].sql).toContain("deleted_at = NULL");
  });

  it("rejects an inconsistent first-message transaction", async () => {
    await expect(
      createChatGroupWithMessage({
        groupId: "other-group",
        ownerUserId: "user-1",
        title: "Hello",
        createdAt: "2026-07-10T10:00:00.000Z",
        message,
      }),
    ).rejects.toThrow("does not match");
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("upserts messages without replacing their original creation time", async () => {
    await upsertChatMessage(message);

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    const conflictUpdate = statement.sql.split("ON CONFLICT(id)")[1];
    expect(conflictUpdate).not.toContain("created_at =");
    expect(statement.params).toContain(message.createdAt);
  });

  it("only replaces a generated title while the fallback is current", async () => {
    await setChatGroupTitleIfCurrent({
      groupId: "group-1",
      expectedTitle: "Fallback",
      title: "Generated",
    });

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("title = ? AND deleted_at IS NULL");
    expect(statement.params).toEqual([
      "Generated",
      expect.any(String),
      "group-1",
      "Fallback",
    ]);
  });

  it("resolves the persisted group for a submitted message", async () => {
    mocks.execute.mockResolvedValue([{ chat_group_id: "group-1" }]);

    await expect(getChatMessageGroupId("message-1")).resolves.toBe("group-1");
    expect(mocks.execute).toHaveBeenCalledWith(expect.any(String), [
      "message-1",
    ]);
  });

  it("uses tombstones for one-message and reconciliation deletes", async () => {
    await deleteChatMessage("group-1", "message-1");
    await deleteChatMessagesExcept("group-1", ["message-2"]);

    const first = mocks.executeTransaction.mock.calls[0][0][0];
    const second = mocks.executeTransaction.mock.calls[1][0][0];
    expect(first.sql).toContain("SET deleted_at = ?");
    expect(first.params.slice(-2)).toEqual(["message-1", "group-1"]);
    expect(second.sql).toContain("json_each(?)");
    expect(second.params.at(-1)).toBe('["message-2"]');
  });
});
