import { describe, expect, test, vi } from "vitest";

import { buildChatSaveOps } from "./save";
import type { ChatJson } from "./types";

import type { TablesContent } from "~/store/tinybase/persister/shared";

type JsonOp = { type: "write-json"; path: string; content: ChatJson };

vi.mock("@tauri-apps/api/path", () => ({
  sep: () => "/",
}));

describe("buildChatSaveOps", () => {
  const dataDir = "/data";

  describe("full save (no changedGroupIds)", () => {
    test("creates json operation for group with messages", () => {
      const tables: TablesContent = {
        chat_groups: {
          "group-1": {
            user_id: "user-1",
            created_at: "2024-01-01",
            title: "Chat",
          },
        },
        chat_messages: {
          "msg-1": {
            user_id: "user-1",
            created_at: "2024-01-01T00:00:01Z",
            chat_group_id: "group-1",
            role: "user",
            content: "Hello",
            metadata: "{}",
            parts: "[]",
          },
        },
      };

      const ops = buildChatSaveOps(tables, dataDir);

      expect(ops).toHaveLength(1);
      expect(ops[0]).toEqual({
        type: "write-json",
        path: "/data/chats/group-1/messages.json",
        content: {
          chat_group: {
            id: "group-1",
            user_id: "user-1",
            created_at: "2024-01-01",
            title: "Chat",
          },
          messages: [
            {
              id: "msg-1",
              user_id: "user-1",
              created_at: "2024-01-01T00:00:01Z",
              chat_group_id: "group-1",
              role: "user",
              content: "Hello",
              metadata: "{}",
              parts: "[]",
            },
          ],
        },
      });
    });

    test("creates json operation with empty messages for group without messages", () => {
      const tables: TablesContent = {
        chat_groups: {
          "group-1": {
            user_id: "user-1",
            created_at: "2024-01-01",
            title: "Empty Chat",
          },
        },
        chat_messages: {},
      };

      const ops = buildChatSaveOps(tables, dataDir);

      expect(ops).toHaveLength(1);
      expect(ops[0]).toEqual({
        type: "write-json",
        path: "/data/chats/group-1/messages.json",
        content: {
          chat_group: {
            id: "group-1",
            user_id: "user-1",
            created_at: "2024-01-01",
            title: "Empty Chat",
          },
          messages: [],
        },
      });
    });

    test("handles multiple groups", () => {
      const tables: TablesContent = {
        chat_groups: {
          "group-1": {
            user_id: "user-1",
            created_at: "2024-01-01",
            title: "Chat 1",
          },
          "group-2": {
            user_id: "user-1",
            created_at: "2024-01-02",
            title: "Chat 2",
          },
        },
        chat_messages: {
          "msg-1": {
            user_id: "user-1",
            created_at: "2024-01-01T00:00:01Z",
            chat_group_id: "group-1",
            role: "user",
            content: "Hello",
            metadata: "{}",
            parts: "[]",
          },
        },
      };

      const ops = buildChatSaveOps(tables, dataDir);

      expect(ops).toHaveLength(2);

      const group1Op = ops.find(
        (op) => op.type === "write-json" && op.path.includes("group-1"),
      ) as JsonOp;
      const group2Op = ops.find(
        (op) => op.type === "write-json" && op.path.includes("group-2"),
      ) as JsonOp;

      expect(group1Op.content.messages).toHaveLength(1);
      expect(group2Op.content.messages).toHaveLength(0);
    });

    test("returns empty array for empty tables", () => {
      const tables: TablesContent = {};
      const ops = buildChatSaveOps(tables, dataDir);
      expect(ops).toHaveLength(0);
    });
  });

  describe("incremental save (with changedGroupIds)", () => {
    test("only saves changed groups", () => {
      const tables: TablesContent = {
        chat_groups: {
          "group-1": {
            user_id: "user-1",
            created_at: "2024-01-01",
            title: "Chat 1",
          },
          "group-2": {
            user_id: "user-1",
            created_at: "2024-01-02",
            title: "Chat 2",
          },
        },
        chat_messages: {
          "msg-1": {
            user_id: "user-1",
            created_at: "2024-01-01T00:00:01Z",
            chat_group_id: "group-1",
            role: "user",
            content: "Hello",
            metadata: "{}",
            parts: "[]",
          },
          "msg-2": {
            user_id: "user-1",
            created_at: "2024-01-02T00:00:01Z",
            chat_group_id: "group-2",
            role: "user",
            content: "Hi",
            metadata: "{}",
            parts: "[]",
          },
        },
      };

      const ops = buildChatSaveOps(tables, dataDir, new Set(["group-1"]));

      expect(ops).toHaveLength(1);
      expect(ops[0].type).toBe("write-json");
      expect((ops[0] as JsonOp).path).toContain("group-1");
    });

    test("creates delete-batch operation for deleted groups", () => {
      const tables: TablesContent = {
        chat_groups: {},
        chat_messages: {},
      };

      const ops = buildChatSaveOps(
        tables,
        dataDir,
        new Set(["deleted-group-1", "deleted-group-2"]),
      );

      expect(ops).toHaveLength(1);
      expect(ops[0]).toEqual({
        type: "delete",
        paths: [
          "/data/chats/deleted-group-1/messages.json",
          "/data/chats/deleted-group-2/messages.json",
        ],
      });
    });

    test("handles mix of modified, empty, and deleted groups", () => {
      const tables: TablesContent = {
        chat_groups: {
          "group-1": {
            user_id: "user-1",
            created_at: "2024-01-01",
            title: "Chat with messages",
          },
          "group-2": {
            user_id: "user-1",
            created_at: "2024-01-02",
            title: "Empty chat",
          },
        },
        chat_messages: {
          "msg-1": {
            user_id: "user-1",
            created_at: "2024-01-01T00:00:01Z",
            chat_group_id: "group-1",
            role: "user",
            content: "Hello",
            metadata: "{}",
            parts: "[]",
          },
        },
      };

      const ops = buildChatSaveOps(
        tables,
        dataDir,
        new Set(["group-1", "group-2", "deleted-group"]),
      );

      expect(ops).toHaveLength(3);

      const jsonOps = ops.filter((op) => op.type === "write-json");
      const deleteOps = ops.filter((op) => op.type === "delete");

      expect(jsonOps).toHaveLength(2);
      expect(deleteOps).toHaveLength(1);

      const group1Op = jsonOps.find((op) =>
        (op as JsonOp).path.includes("group-1"),
      ) as JsonOp;
      const group2Op = jsonOps.find((op) =>
        (op as JsonOp).path.includes("group-2"),
      ) as JsonOp;

      expect(group1Op.content.messages).toHaveLength(1);
      expect(group2Op.content.messages).toHaveLength(0);

      expect(deleteOps[0]).toEqual({
        type: "delete",
        paths: ["/data/chats/deleted-group/messages.json"],
      });
    });

    test("returns empty array when changed groups set is empty", () => {
      const tables: TablesContent = {
        chat_groups: {
          "group-1": {
            user_id: "user-1",
            created_at: "2024-01-01",
            title: "Chat",
          },
        },
      };

      const ops = buildChatSaveOps(tables, dataDir, new Set());

      expect(ops).toHaveLength(0);
    });
  });
});
