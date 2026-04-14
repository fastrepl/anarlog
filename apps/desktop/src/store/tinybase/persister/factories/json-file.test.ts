import { beforeEach, describe, expect, test, vi } from "vitest";

import { createJsonFilePersister } from "./json-file";

import {
  createTestMainStore,
  MOCK_DATA_DIR,
} from "~/store/tinybase/persister/testing/mocks";

const settingsMocks = vi.hoisted(() => ({
  vaultBase: vi
    .fn()
    .mockResolvedValue({ status: "ok", data: "/mock/data/dir/hyprnote" }),
}));

const fs2Mocks = vi.hoisted(() => ({
  readTextFile: vi.fn(),
  remove: vi.fn(),
}));

const fsSyncMocks = vi.hoisted(() => ({
  writeJsonBatch: vi.fn().mockResolvedValue({ status: "ok" }),
}));

const notifyMocks = vi.hoisted(() => ({
  fileChanged: {
    listen: vi.fn().mockResolvedValue(() => {}),
  },
}));

vi.mock("@hypr/plugin-settings", () => ({ commands: settingsMocks }));
vi.mock("@hypr/plugin-fs2", () => ({ commands: fs2Mocks }));
vi.mock("@hypr/plugin-fs-sync", () => ({ commands: fsSyncMocks }));
vi.mock("@hypr/plugin-notify", () => ({ events: notifyMocks }));

describe("createJsonFilePersister", () => {
  let store: ReturnType<typeof createTestMainStore>;

  beforeEach(() => {
    store = createTestMainStore();
    vi.clearAllMocks();
  });

  test("returns a persister object with expected methods", () => {
    const persister = createJsonFilePersister(store, {
      tableName: "chat_messages",
      filename: "test.json",
      label: "test",
    });

    expect(persister).toBeDefined();
    expect(persister.save).toBeTypeOf("function");
    expect(persister.load).toBeTypeOf("function");
    expect(persister.destroy).toBeTypeOf("function");
  });

  describe("load", () => {
    test("loads data from json file into specified table", async () => {
      const mockData = {
        "item-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
        },
      };
      fs2Mocks.readTextFile.mockResolvedValue({
        status: "ok",
        data: JSON.stringify(mockData),
      });

      const persister = createJsonFilePersister(store, {
        tableName: "chat_messages",
        filename: "test.json",
        label: "test",
      });
      await persister.load();

      expect(fs2Mocks.readTextFile).toHaveBeenCalledWith(
        `${MOCK_DATA_DIR}/test.json`,
      );
      expect(store.getTable("chat_messages")).toEqual(mockData);
    });

    test("handles file not found gracefully", async () => {
      fs2Mocks.readTextFile.mockResolvedValue({
        status: "error",
        error: "No such file or directory",
      });

      const persister = createJsonFilePersister(store, {
        tableName: "chat_messages",
        filename: "nonexistent.json",
        label: "test",
      });
      await persister.load();

      expect(store.getTable("chat_messages")).toEqual({});
    });
  });

  describe("save", () => {
    test("saves table data to json file", async () => {
      store.setRow("chat_messages", "item-1", {
        user_id: "user-1",
        created_at: "2024-01-01T00:00:00Z",
        chat_group_id: "group-1",
        role: "user",
        content: "Test message",
        metadata: "",
        parts: "",
        status: "sent",
      });

      const persister = createJsonFilePersister(store, {
        tableName: "chat_messages",
        filename: "test.json",
        label: "test",
      });
      await persister.save();

      expect(fsSyncMocks.writeJsonBatch).toHaveBeenCalledWith([
        [expect.any(Object), `${MOCK_DATA_DIR}/test.json`],
      ]);

      const writtenData = fsSyncMocks.writeJsonBatch.mock.calls[0][0][0][0];
      expect(writtenData["item-1"].content).toBe("Test message");
    });

    test("writes empty object when table is empty", async () => {
      const persister = createJsonFilePersister(store, {
        tableName: "chat_messages",
        filename: "test.json",
        label: "test",
      });
      await persister.save();

      expect(fsSyncMocks.writeJsonBatch).toHaveBeenCalledWith([
        [{}, `${MOCK_DATA_DIR}/test.json`],
      ]);
    });
  });
});
