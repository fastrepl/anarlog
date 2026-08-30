import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createFolder: vi.fn(),
  deleteFolder: vi.fn(),
  execute: vi.fn(),
  executeTransaction: vi.fn().mockResolvedValue([0, 1]),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<unknown>) => write(),
  ),
  moveSession: vi.fn(),
  renameFolder: vi.fn(),
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    createFolder: mocks.createFolder,
    deleteFolder: mocks.deleteFolder,
    moveSession: mocks.moveSession,
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
  deleteNamedFolder,
  ensureFolderCatalog,
  renameNamedFolder,
  updateFolderInstructions,
} from "./folder-catalog";

describe("folder catalog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.executeTransaction.mockResolvedValue([0, 1]);
    mocks.execute.mockResolvedValue([]);
    mocks.createFolder.mockResolvedValue({ status: "ok", data: null });
    mocks.deleteFolder.mockResolvedValue({ status: "ok", data: null });
    mocks.moveSession.mockResolvedValue({
      status: "ok",
      data: { session_id: "session-1", folder_id: "" },
    });
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

  it("persists ancestor catalog rows for a nested folder", async () => {
    await expect(createNamedFolder("CS 101/Week 1")).resolves.toBe(
      "CS 101/Week 1",
    );

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    const insertParams = statements
      .filter((statement: { sql: string }) =>
        statement.sql.includes("INSERT INTO folders"),
      )
      .flatMap((statement: { params: string[] }) => statement.params);
    expect(insertParams).toContain("CS 101");
    expect(insertParams).toContain("CS 101/Week 1");
    expect(mocks.createFolder).toHaveBeenCalledWith("CS 101/Week 1");
  });

  it("moves notes out, tombstones the tree, and deletes the disk folder", async () => {
    mocks.execute.mockResolvedValue([
      { id: "session-1", folder_path: "CS 101" },
      { id: "session-2", folder_path: "CS 101/Week 1" },
    ]);

    await deleteNamedFolder("CS 101");

    expect(mocks.moveSession).toHaveBeenCalledWith("session-1", "CS 101", "");
    expect(mocks.moveSession).toHaveBeenCalledWith(
      "session-2",
      "CS 101/Week 1",
      "",
    );
    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements[0].sql).toContain("UPDATE folders");
    expect(statements[0].sql).toContain("deleted_at");
    expect(statements[1].sql).toContain("UPDATE folder_attachments");
    expect(statements[2].sql).toContain("UPDATE sessions");
    expect(statements[2].sql).toContain("folder_path = ''");
    expect(mocks.deleteFolder).toHaveBeenCalledWith("CS 101");
  });

  it("saves folder chat instructions on the catalog row", async () => {
    await updateFolderInstructions("CS 101", "Prefer the syllabus.");

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(2);
    const update = mocks.executeTransaction.mock.calls[1]![0][0];
    expect(update.sql).toContain("SET");
    expect(update.sql).toContain("instructions = ?");
    expect(update.params).toEqual(["Prefer the syllabus.", "CS 101"]);
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
