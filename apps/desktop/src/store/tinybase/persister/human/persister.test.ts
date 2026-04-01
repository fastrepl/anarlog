import { beforeEach, describe, expect, test, vi } from "vitest";

import { createHumanPersister } from "./persister";

import {
  createTestMainStore,
  MOCK_DATA_DIR,
  TEST_UUID_1,
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

describe("createHumanPersister", () => {
  let store: ReturnType<typeof createTestMainStore>;

  beforeEach(() => {
    store = createTestMainStore();
    vi.clearAllMocks();
  });

  test("returns a persister object with expected methods", () => {
    const persister = createHumanPersister(store);

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

    const persister = createHumanPersister(store);
    await persister.load();

    expect(fsSyncMocks.readDocumentBatch).toHaveBeenCalledWith(
      `${MOCK_DATA_DIR}/humans`,
    );
  });

  describe("email transform integration", () => {
    test("loads with emails array (new format)", async () => {
      fsSyncMocks.readDocumentBatch.mockResolvedValue({
        status: "ok",
        data: {
          [TEST_UUID_1]: {
            frontmatter: {
              user_id: "user-1",
              created_at: "2024-01-01T00:00:00Z",
              name: "John Doe",
              emails: ["john@example.com", "john.doe@work.com"],
              org_id: "org-1",
              job_title: "Engineer",
              linkedin_username: "johndoe",
            },
            content: "",
          },
        },
      });

      const persister = createHumanPersister(store);
      await persister.load();

      const humans = store.getTable("humans");
      expect(humans[TEST_UUID_1]?.email).toBe(
        "john@example.com,john.doe@work.com",
      );
    });

    test("loads with legacy email string (backward compat)", async () => {
      fsSyncMocks.readDocumentBatch.mockResolvedValue({
        status: "ok",
        data: {
          [TEST_UUID_1]: {
            frontmatter: {
              user_id: "user-1",
              created_at: "2024-01-01T00:00:00Z",
              name: "John Doe",
              email: "john@example.com",
              org_id: "org-1",
              job_title: "Engineer",
              linkedin_username: "johndoe",
            },
            content: "",
          },
        },
      });

      const persister = createHumanPersister(store);
      await persister.load();

      const humans = store.getTable("humans");
      expect(humans[TEST_UUID_1]?.email).toBe("john@example.com");
    });

    test("saves multiple emails as array", async () => {
      store.setRow("humans", TEST_UUID_1, {
        user_id: "user-1",
        name: "John Doe",
        email: "john@example.com,john.doe@work.com",
        org_id: "org-1",
        job_title: "Engineer",
        linkedin_username: "johndoe",
        memo: "",
      });

      const persister = createHumanPersister(store);
      await persister.save();

      const batchItems = fsSyncMocks.writeDocumentBatch.mock.calls[0][0];
      const frontmatter = batchItems[0][0].frontmatter;

      expect(frontmatter.emails).toEqual([
        "john@example.com",
        "john.doe@work.com",
      ]);
      expect(frontmatter.email).toBeUndefined();
    });
  });
});
