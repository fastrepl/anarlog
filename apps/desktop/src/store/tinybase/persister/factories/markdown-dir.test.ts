import { beforeEach, describe, expect, test, vi } from "vitest";

import type { JsonValue } from "@hypr/plugin-fs-sync";

import { createMarkdownDirPersister } from "./markdown-dir";

import {
  createTestMainStore,
  MOCK_DATA_DIR,
  TEST_UUID_1,
  TEST_UUID_2,
} from "~/store/tinybase/persister/testing/mocks";

const settingsMocks = vi.hoisted(() => ({
  vaultBase: vi
    .fn()
    .mockResolvedValue({ status: "ok", data: "/mock/data/dir/hyprnote" }),
}));

const fsSyncMocks = vi.hoisted(() => ({
  deserialize: vi.fn(),
  serialize: vi.fn().mockResolvedValue({ status: "ok", data: "" }),
  writeDocumentBatch: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  readDocumentBatch: vi.fn(),
  cleanupOrphan: vi.fn().mockResolvedValue({ status: "ok", data: 0 }),
}));

const fs2Mocks = vi.hoisted(() => ({
  readTextFile: vi.fn(),
  remove: vi.fn(),
}));

vi.mock("@hypr/plugin-settings", () => ({ commands: settingsMocks }));
vi.mock("@hypr/plugin-fs-sync", () => ({ commands: fsSyncMocks }));
vi.mock("@hypr/plugin-fs2", () => ({ commands: fs2Mocks }));

const testConfig = {
  tableName: "humans",
  dirName: "humans",
  label: "TestHumanPersister",
  entityParser: (path: string) => {
    const match = path.match(/humans\/([^/]+)\.md$/);
    return match ? match[1] : null;
  },
  toFrontmatter: (entity: Record<string, unknown>) => ({
    frontmatter: entity as Record<string, JsonValue>,
    body: "",
  }),
  fromFrontmatter: (frontmatter: Record<string, unknown>) => frontmatter,
};

describe("createMarkdownDirPersister", () => {
  let store: ReturnType<typeof createTestMainStore>;

  beforeEach(() => {
    store = createTestMainStore();
    vi.clearAllMocks();
  });

  test("returns a persister object with expected methods", () => {
    const persister = createMarkdownDirPersister(store, testConfig);

    expect(persister).toBeDefined();
    expect(persister.save).toBeTypeOf("function");
    expect(persister.load).toBeTypeOf("function");
    expect(persister.destroy).toBeTypeOf("function");
  });

  describe("load", () => {
    test("loads entities from markdown files", async () => {
      fsSyncMocks.readDocumentBatch.mockResolvedValue({
        status: "ok",
        data: {
          [TEST_UUID_1]: {
            frontmatter: {
              user_id: "user-1",
              name: "John Doe",
              email: "john@example.com",
              org_id: "",
            },
            content: "",
          },
        },
      });

      const persister = createMarkdownDirPersister(store, testConfig);
      await persister.load();

      expect(fsSyncMocks.readDocumentBatch).toHaveBeenCalledWith(
        `${MOCK_DATA_DIR}/humans`,
      );

      const entities = store.getTable("humans");
      expect(entities[TEST_UUID_1]).toEqual({
        user_id: "user-1",
        name: "John Doe",
        email: "john@example.com",
        org_id: "",
      });
    });

    test("loads multiple entities", async () => {
      fsSyncMocks.readDocumentBatch.mockResolvedValue({
        status: "ok",
        data: {
          [TEST_UUID_1]: {
            frontmatter: {
              user_id: "user-1",
              name: "John Doe",
              email: "john@example.com",
              org_id: "",
            },
            content: "",
          },
          [TEST_UUID_2]: {
            frontmatter: {
              user_id: "user-1",
              name: "Jane Doe",
              email: "jane@example.com",
              org_id: "",
            },
            content: "",
          },
        },
      });

      const persister = createMarkdownDirPersister(store, testConfig);
      await persister.load();

      const entities = store.getTable("humans");
      expect(Object.keys(entities)).toHaveLength(2);
      expect(entities[TEST_UUID_1]).toBeDefined();
      expect(entities[TEST_UUID_2]).toBeDefined();
    });

    test("returns empty when directory does not exist", async () => {
      fsSyncMocks.readDocumentBatch.mockResolvedValue({
        status: "error",
        error: "No such file or directory",
      });

      const persister = createMarkdownDirPersister(store, testConfig);
      await persister.load();

      expect(store.getTable("humans")).toEqual({});
    });
  });

  describe("save", () => {
    test("saves entity to markdown file via batch write", async () => {
      store.setRow("humans", TEST_UUID_1, {
        user_id: "user-1",
        name: "John Doe",
        email: "john@example.com",
        org_id: "",
      });

      const persister = createMarkdownDirPersister(store, testConfig);
      await persister.save();

      expect(fsSyncMocks.writeDocumentBatch).toHaveBeenCalledWith([
        [
          {
            frontmatter: {
              user_id: "user-1",
              name: "John Doe",
              email: "john@example.com",
              org_id: "",
            },
            content: "",
          },
          `${MOCK_DATA_DIR}/humans/${TEST_UUID_1}.md`,
        ],
      ]);
    });

    test("does not write when no entities exist", async () => {
      const persister = createMarkdownDirPersister(store, testConfig);
      await persister.save();

      expect(fsSyncMocks.writeDocumentBatch).not.toHaveBeenCalled();
    });

    test("saves multiple entities in single batch call", async () => {
      store.setRow("humans", TEST_UUID_1, {
        user_id: "user-1",
        name: "John Doe",
        email: "john@example.com",
        org_id: "",
      });

      store.setRow("humans", TEST_UUID_2, {
        user_id: "user-1",
        name: "Jane Doe",
        email: "jane@example.com",
        org_id: "",
      });

      const persister = createMarkdownDirPersister(store, testConfig);
      await persister.save();

      expect(fsSyncMocks.writeDocumentBatch).toHaveBeenCalledTimes(1);

      const batchItems = fsSyncMocks.writeDocumentBatch.mock.calls[0][0];
      expect(batchItems).toHaveLength(2);

      const paths = batchItems.map((item: [unknown, string]) => item[1]);
      expect(paths).toContain(`${MOCK_DATA_DIR}/humans/${TEST_UUID_1}.md`);
      expect(paths).toContain(`${MOCK_DATA_DIR}/humans/${TEST_UUID_2}.md`);
    });
  });
});
