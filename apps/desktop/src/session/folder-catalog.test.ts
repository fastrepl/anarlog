import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createFolder: vi.fn(),
  deleteFolder: vi.fn(),
  deleteFolderCatalog: vi.fn(),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<unknown>) => write(),
  ),
  ensureFolderCatalog: vi.fn(),
  execute: vi.fn(),
  moveSession: vi.fn(),
  renameFolder: vi.fn(),
  renameFolderCatalog: vi.fn(),
  updateFolderIcon: vi.fn(),
  updateFolderInstructions: vi.fn(),
  updateFolderWorkspace: vi.fn(),
}));

vi.mock("@anlg/plugin-session", () => ({
  commands: {
    deleteFolderCatalog: mocks.deleteFolderCatalog,
    ensureFolderCatalog: mocks.ensureFolderCatalog,
    renameFolderCatalog: mocks.renameFolderCatalog,
    updateFolderIcon: mocks.updateFolderIcon,
    updateFolderInstructions: mocks.updateFolderInstructions,
    updateFolderWorkspace: mocks.updateFolderWorkspace,
  },
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
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: () => ({ data: [] }),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
}));

import {
  createNamedFolder,
  deleteNamedFolder,
  ensureFolderCatalog,
  loadFolderWorkspaceId,
  renameNamedFolder,
} from "./folder-catalog";

describe("folder catalog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    for (const command of [
      mocks.deleteFolderCatalog,
      mocks.ensureFolderCatalog,
      mocks.renameFolderCatalog,
      mocks.updateFolderIcon,
      mocks.updateFolderInstructions,
      mocks.updateFolderWorkspace,
    ]) {
      command.mockResolvedValue({ status: "ok", data: null });
    }
    mocks.execute.mockResolvedValue([]);
    mocks.createFolder.mockResolvedValue({ status: "ok", data: null });
    mocks.deleteFolder.mockResolvedValue({ status: "ok", data: null });
    mocks.moveSession.mockResolvedValue({ status: "ok", data: null });
    mocks.renameFolder.mockResolvedValue({ status: "ok", data: null });
  });

  it("creates a named folder", async () => {
    await expect(createNamedFolder("CS 101")).resolves.toBe("CS 101");
  });

  it("rejects the unfiled folder", async () => {
    await expect(ensureFolderCatalog("")).rejects.toThrow(
      "invalid folder path",
    );
    expect(mocks.ensureFolderCatalog).not.toHaveBeenCalled();
  });

  it("refuses to rename onto an existing folder", async () => {
    mocks.execute.mockResolvedValue([{ present: 1 }]);

    await expect(renameNamedFolder("CS 101", "work")).rejects.toThrow(
      "folder_target_exists",
    );
    expect(mocks.renameFolder).not.toHaveBeenCalled();
    expect(mocks.renameFolderCatalog).not.toHaveBeenCalled();
  });

  it("renames a folder", async () => {
    await expect(renameNamedFolder("CS 101", "Algorithms")).resolves.toBe(
      "Algorithms",
    );
  });

  it("does not delete a folder if moving a note fails", async () => {
    mocks.execute.mockResolvedValue([
      { id: "session-1", folder_path: "CS 101" },
    ]);
    mocks.moveSession.mockResolvedValue({
      status: "error",
      error: "permission denied",
    });

    await expect(deleteNamedFolder("CS 101")).rejects.toThrow(
      "permission denied",
    );
    expect(mocks.deleteFolderCatalog).not.toHaveBeenCalled();
    expect(mocks.deleteFolder).not.toHaveBeenCalled();
  });

  it("does not update the catalog if creating a missing rename target fails", async () => {
    mocks.renameFolder.mockResolvedValue({
      status: "error",
      error: "Path error: folder_source_missing",
    });
    mocks.createFolder.mockResolvedValue({
      status: "error",
      error: "permission denied",
    });

    await expect(renameNamedFolder("Empty", "Named")).rejects.toThrow(
      "permission denied",
    );
    expect(mocks.renameFolderCatalog).not.toHaveBeenCalled();
  });

  it("propagates filesystem rename errors without updating the catalog", async () => {
    mocks.renameFolder.mockResolvedValue({
      status: "error",
      error: "permission denied",
    });

    await expect(renameNamedFolder("Existing", "New")).rejects.toThrow(
      "permission denied",
    );
    expect(mocks.renameFolderCatalog).not.toHaveBeenCalled();
  });

  it("maps a missing workspace to an empty id", async () => {
    await expect(loadFolderWorkspaceId("CS 101")).resolves.toBe("");
  });
});
