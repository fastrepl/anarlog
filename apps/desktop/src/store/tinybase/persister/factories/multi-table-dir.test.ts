import { beforeEach, describe, expect, test, vi } from "vitest";

import { createMultiTableDirPersister } from "./multi-table-dir";

import { ok } from "~/store/tinybase/persister/shared/load-result";
import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

const settingsMocks = vi.hoisted(() => ({
  base: vi
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

describe("createMultiTableDirPersister", () => {
  let store: ReturnType<typeof createTestMainStore>;

  beforeEach(() => {
    store = createTestMainStore();
    vi.clearAllMocks();
  });

  test("returns a persister object with expected methods", () => {
    const persister = createMultiTableDirPersister(store, {
      label: "TestPersister",
      dirName: "test",
      entityParser: () => null,
      tables: [{ tableName: "sessions", isPrimary: true }],
      cleanup: () => [],
      loadAll: async () => ok({ sessions: {} }),
      loadSingle: async () => ok({ sessions: {} }),
      save: () => ({ operations: [] }),
    });

    expect(persister).toBeDefined();
    expect(persister.save).toBeTypeOf("function");
    expect(persister.load).toBeTypeOf("function");
    expect(persister.destroy).toBeTypeOf("function");
  });
});
