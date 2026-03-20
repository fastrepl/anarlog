import { beforeEach, describe, expect, test, vi } from "vitest";

import { createOrganizationPersister } from "./persister";

import {
  createTestMainStore,
  MOCK_DATA_DIR,
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

describe("createOrganizationPersister", () => {
  let store: ReturnType<typeof createTestMainStore>;

  beforeEach(() => {
    store = createTestMainStore();
    vi.clearAllMocks();
  });

  test("returns a persister object with expected methods", () => {
    const persister = createOrganizationPersister(store);

    expect(persister).toBeDefined();
    expect(persister.save).toBeTypeOf("function");
    expect(persister.load).toBeTypeOf("function");
    expect(persister.destroy).toBeTypeOf("function");
  });

  test("configures correct table and directory names", async () => {
    fsSyncMocks.readDocumentBatch.mockResolvedValue({
      status: "ok",
      data: {},
    });

    const persister = createOrganizationPersister(store);
    await persister.load();

    expect(fsSyncMocks.readDocumentBatch).toHaveBeenCalledWith(
      `${MOCK_DATA_DIR}/organizations`,
    );
  });
});
