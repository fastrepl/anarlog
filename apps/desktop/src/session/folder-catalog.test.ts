import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createFolder: vi.fn(),
  execute: vi.fn(),
  executeTransaction: vi.fn().mockResolvedValue([0, 1]),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<unknown>) => write(),
  ),
  renameFolder: vi.fn(),
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    createFolder: mocks.createFolder,
    renameFolder: mocks.renameFolder,
  },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
}));

import {
  createNamedFolder,
  ensureFolderCatalog,
  renameNamedFolder,
} from "./folder-catalog";

describe("folder catalog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.executeTransaction.mockResolvedValue([0, 1]);
    mocks.execute.mockResolvedValue([]);
    mocks.createFolder.mockResolvedValue({ status: "ok", data: null });
    mocks.renameFolder.mockResolvedValue({
      status: "ok",
      data: { updates: [] },
    });
  });

  it("persists an empty folder in the catalog and on disk", async () => {
    await expect(createNamedFolder("CS 101")).resolves.toBe("CS 101");

    expect(mocks.enqueueDatabaseWrite).toHaveBeenCalledWith(
      "folders",
      expect.any(Function),
    );
    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements[1].sql).toContain("INSERT INTO folders");
    expect(statements[1].params).toContain("CS 101");
    expect(mocks.createFolder).toHaveBeenCalledWith("CS 101");
  });

  it("rejects the unfiled folder", async () => {
    await expect(ensureFolderCatalog("")).rejects.toThrow(
      "invalid folder path",
    );
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("rewrites catalog, materials, and notes when renaming", async () => {
    await expect(renameNamedFolder("CS 101", "Algorithms")).resolves.toBe(
      "Algorithms",
    );

    expect(mocks.renameFolder).toHaveBeenCalledWith("CS 101", "Algorithms");
    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements[0].sql).toContain("UPDATE folders");
    expect(statements[0].params).toEqual(["Algorithms", "CS 101"]);
    expect(statements[3].sql).toContain("UPDATE folder_attachments");
    expect(statements[4].sql).toContain("UPDATE sessions");
    expect(statements[4].sql).toContain("substr(folder_path");
    expect(statements[4].params).toEqual([
      "CS 101",
      "Algorithms",
      "Algorithms",
      "CS 101",
      "CS 101",
      "CS 101/%",
      "CS 101\\%",
    ]);
  });

  it("refuses to rename onto an existing folder", async () => {
    mocks.execute.mockResolvedValue([{ present: 1 }]);

    await expect(renameNamedFolder("CS 101", "work")).rejects.toThrow(
      "folder_target_exists",
    );
    expect(mocks.renameFolder).not.toHaveBeenCalled();
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("creates the target directory when the source is missing", async () => {
    mocks.renameFolder.mockResolvedValue({
      status: "error",
      error: "Path error: folder_source_missing",
    });

    await expect(renameNamedFolder("Empty", "Named")).resolves.toBe("Named");
    expect(mocks.createFolder).toHaveBeenCalledWith("Named");
    expect(mocks.executeTransaction).toHaveBeenCalled();
  });
});
