import { describe, expect, test } from "vitest";

import { getChangedChatGroupIds, parseChatGroupIdFromPath } from "./changes";

import type {
  ChangedTables,
  TablesContent,
} from "~/store/tinybase/persister/shared";

describe("parseChatGroupIdFromPath", () => {
  describe("relative paths (from notify events)", () => {
    test("extracts chat group ID from valid path", () => {
      expect(parseChatGroupIdFromPath("chats/group-1/messages.json")).toBe(
        "group-1",
      );
    });

    test("extracts uuid chat group ID", () => {
      expect(
        parseChatGroupIdFromPath(
          "chats/550e8400-e29b-41d4-a716-446655440000/messages.json",
        ),
      ).toBe("550e8400-e29b-41d4-a716-446655440000");
    });

    test("extracts chat group ID with any file in group directory", () => {
      expect(parseChatGroupIdFromPath("chats/group-1/any-file.txt")).toBe(
        "group-1",
      );
    });
  });

  describe("edge cases", () => {
    test("returns null when path has no chats segment", () => {
      expect(parseChatGroupIdFromPath("sessions/session-1/file")).toBeNull();
    });

    test("returns null when chats is the last segment", () => {
      expect(parseChatGroupIdFromPath("chats")).toBeNull();
    });

    test("returns null for empty string", () => {
      expect(parseChatGroupIdFromPath("")).toBeNull();
    });

    test("returns null for path ending with chats/", () => {
      expect(parseChatGroupIdFromPath("chats/")).toBeNull();
    });
  });

  describe("absolute paths (defensive handling)", () => {
    test("extracts chat group ID from absolute path", () => {
      expect(
        parseChatGroupIdFromPath(
          "/Users/test/data/hyprnote/chats/abc-123/file",
        ),
      ).toBe("abc-123");
    });
  });
});

describe("getChangedChatGroupIds", () => {
  test("returns changed chat group IDs when groups are directly changed", () => {
    const tables: TablesContent = {
      chat_groups: {
        "group-1": { user_id: "user-1", created_at: "2024-01-01", title: "A" },
      },
    };
    const changedTables: ChangedTables = {
      chat_groups: { "group-1": {} },
    };

    const result = getChangedChatGroupIds(tables, changedTables);

    expect(result).toBeDefined();
    expect(result!.changedChatGroupIds).toEqual(new Set(["group-1"]));
    expect(result!.hasUnresolvedDeletions).toBe(false);
  });

  test("returns changed chat group IDs when messages are changed", () => {
    const tables: TablesContent = {
      chat_groups: {
        "group-1": { user_id: "user-1", created_at: "2024-01-01", title: "A" },
      },
      chat_messages: {
        "msg-1": {
          user_id: "user-1",
          created_at: "2024-01-01",
          chat_group_id: "group-1",
          role: "user",
          content: "Hello",
          metadata: "{}",
          parts: "[]",
        },
      },
    };
    const changedTables: ChangedTables = {
      chat_messages: { "msg-1": {} },
    };

    const result = getChangedChatGroupIds(tables, changedTables);

    expect(result).toBeDefined();
    expect(result!.changedChatGroupIds).toEqual(new Set(["group-1"]));
    expect(result!.hasUnresolvedDeletions).toBe(false);
  });

  test("sets hasUnresolvedDeletions when changed message cannot be resolved", () => {
    const tables: TablesContent = {
      chat_groups: {},
      chat_messages: {},
    };
    const changedTables: ChangedTables = {
      chat_messages: { "deleted-msg": {} },
    };

    const result = getChangedChatGroupIds(tables, changedTables);

    expect(result).toBeDefined();
    expect(result!.changedChatGroupIds.size).toBe(0);
    expect(result!.hasUnresolvedDeletions).toBe(true);
  });

  test("returns undefined when no relevant changes", () => {
    const tables: TablesContent = {};
    const changedTables: ChangedTables = {};

    const result = getChangedChatGroupIds(tables, changedTables);

    expect(result).toBeUndefined();
  });

  test("returns undefined when only unrelated tables changed", () => {
    const tables: TablesContent = {};
    const changedTables: ChangedTables = {
      sessions: { "session-1": {} },
    };

    const result = getChangedChatGroupIds(tables, changedTables);

    expect(result).toBeUndefined();
  });

  test("handles mixed changes from groups and messages", () => {
    const tables: TablesContent = {
      chat_groups: {
        "group-1": { user_id: "user-1", created_at: "2024-01-01", title: "A" },
        "group-2": { user_id: "user-1", created_at: "2024-01-01", title: "B" },
      },
      chat_messages: {
        "msg-1": {
          user_id: "user-1",
          created_at: "2024-01-01",
          chat_group_id: "group-2",
          role: "user",
          content: "Hello",
          metadata: "{}",
          parts: "[]",
        },
      },
    };
    const changedTables: ChangedTables = {
      chat_groups: { "group-1": {} },
      chat_messages: { "msg-1": {} },
    };

    const result = getChangedChatGroupIds(tables, changedTables);

    expect(result).toBeDefined();
    expect(result!.changedChatGroupIds).toEqual(
      new Set(["group-1", "group-2"]),
    );
    expect(result!.hasUnresolvedDeletions).toBe(false);
  });

  test("deduplicates when group and its message both changed", () => {
    const tables: TablesContent = {
      chat_groups: {
        "group-1": { user_id: "user-1", created_at: "2024-01-01", title: "A" },
      },
      chat_messages: {
        "msg-1": {
          user_id: "user-1",
          created_at: "2024-01-01",
          chat_group_id: "group-1",
          role: "user",
          content: "Hello",
          metadata: "{}",
          parts: "[]",
        },
      },
    };
    const changedTables: ChangedTables = {
      chat_groups: { "group-1": {} },
      chat_messages: { "msg-1": {} },
    };

    const result = getChangedChatGroupIds(tables, changedTables);

    expect(result).toBeDefined();
    expect(result!.changedChatGroupIds).toEqual(new Set(["group-1"]));
  });
});
