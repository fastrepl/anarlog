import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn().mockResolvedValue([0, 1]),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<number[]>) => write(),
  ),
  folderAttachmentRemove: vi.fn(),
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    folderAttachmentRemove: mocks.folderAttachmentRemove,
  },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: () => ({ data: [] }),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
}));

import {
  catalogLocalFolderMaterial,
  deleteLocalFolderMaterial,
  diskAttachmentId,
  loadFolderMaterial,
  loadFolderMaterials,
} from "./folder-attachments";

describe("folder material catalog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.executeTransaction.mockResolvedValue([0, 1]);
    mocks.execute.mockResolvedValue([]);
    mocks.folderAttachmentRemove.mockResolvedValue({
      status: "ok",
      data: null,
    });
  });

  it("stores materials against the folder path, not a session", async () => {
    await catalogLocalFolderMaterial({
      folderPath: "CS 101",
      attachmentId: "syllabus.txt",
      filename: "syllabus.txt",
      contentType: "text/plain",
      sizeBytes: 12,
      sha256: "a".repeat(64),
    });

    expect(mocks.enqueueDatabaseWrite).toHaveBeenCalledWith(
      "folder:CS 101",
      expect.any(Function),
    );
    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(2);
    expect(statements[1].sql).toContain("INSERT INTO folder_attachments");
    expect(statements[1].sql).toContain("folder_path");
    expect(statements[1].sql).not.toContain("session_id");
    expect(statements[1].sql).not.toContain("attachment_transfer_jobs");
    expect(statements[1].params).toContain("materials/syllabus.txt");
    expect(statements[1].params).toContain("CS 101");
  });

  it("rejects the unfiled folder", async () => {
    await expect(
      catalogLocalFolderMaterial({
        folderPath: "",
        attachmentId: "syllabus.txt",
        filename: "syllabus.txt",
        contentType: "text/plain",
        sizeBytes: 12,
        sha256: "a".repeat(64),
      }),
    ).rejects.toThrow("invalid folder path");
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("loads materials for a named folder", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "mat-1",
        filename: "syllabus.txt",
        content_type: "text/plain",
        size_bytes: 12,
        relative_path: "materials/syllabus.txt",
      },
    ]);

    await expect(loadFolderMaterials("CS 101")).resolves.toEqual([
      {
        id: "mat-1",
        filename: "syllabus.txt",
        contentType: "text/plain",
        sizeBytes: 12,
        relativePath: "materials/syllabus.txt",
      },
    ]);
    expect(mocks.execute).toHaveBeenCalledWith(
      expect.stringContaining("folder_path = ?"),
      ["CS 101"],
    );
  });

  it("loads one material by id", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "mat-1",
        filename: "syllabus.txt",
        content_type: "text/plain",
        size_bytes: 12,
        relative_path: "materials/syllabus.txt",
      },
    ]);

    await expect(loadFolderMaterial("CS 101", "mat-1")).resolves.toEqual({
      id: "mat-1",
      filename: "syllabus.txt",
      contentType: "text/plain",
      sizeBytes: 12,
      relativePath: "materials/syllabus.txt",
    });
  });

  it("tombstones then removes the on-disk file", async () => {
    mocks.executeTransaction.mockResolvedValue([1]);

    await deleteLocalFolderMaterial({
      folderPath: "CS 101",
      attachmentId: "syllabus.txt",
    });

    expect(mocks.executeTransaction.mock.calls[0]![0][0].sql).toContain(
      "deleted_at = strftime",
    );
    expect(mocks.folderAttachmentRemove).toHaveBeenCalledWith(
      "CS 101",
      "syllabus.txt",
    );
  });

  it("reads the disk filename from a relative path", () => {
    expect(diskAttachmentId("materials/syllabus 1.pdf")).toBe("syllabus 1.pdf");
  });
});
