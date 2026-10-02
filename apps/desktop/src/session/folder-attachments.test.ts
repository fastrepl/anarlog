import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  catalogFolderMaterial: vi.fn(),
  enqueueDatabaseWrite: vi.fn(
    async (_key: string, write: () => Promise<unknown>) => write(),
  ),
  execute: vi.fn(),
  folderAttachmentRemove: vi.fn(),
  tombstoneFolderMaterial: vi.fn(),
}));

vi.mock("@anlg/plugin-session", () => ({
  commands: {
    catalogFolderMaterial: mocks.catalogFolderMaterial,
    tombstoneFolderMaterial: mocks.tombstoneFolderMaterial,
  },
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    folderAttachmentRemove: mocks.folderAttachmentRemove,
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
  catalogLocalFolderMaterial,
  deleteLocalFolderMaterial,
  diskAttachmentId,
  loadFolderMaterial,
  loadFolderMaterials,
} from "./folder-attachments";

describe("folder material catalog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.catalogFolderMaterial.mockResolvedValue({ status: "ok", data: null });
    mocks.tombstoneFolderMaterial.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.execute.mockResolvedValue([]);
    mocks.folderAttachmentRemove.mockResolvedValue({
      status: "ok",
      data: null,
    });
  });

  it("rejects the unfiled folder before cataloging", async () => {
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
    expect(mocks.catalogFolderMaterial).not.toHaveBeenCalled();
  });

  it("loads and maps materials for a named folder", async () => {
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

  it("does not remove the on-disk file when tombstoning fails", async () => {
    mocks.tombstoneFolderMaterial.mockResolvedValue({
      status: "error",
      error: "folder material is unavailable",
    });

    await expect(
      deleteLocalFolderMaterial({
        folderPath: "CS 101",
        attachmentId: "syllabus.txt",
      }),
    ).rejects.toThrow("folder material is unavailable");
    expect(mocks.folderAttachmentRemove).not.toHaveBeenCalled();
  });

  it("preserves errors from removing the on-disk file", async () => {
    mocks.folderAttachmentRemove.mockResolvedValue({
      status: "error",
      error: "disk unavailable",
    });

    await expect(
      deleteLocalFolderMaterial({
        folderPath: "CS 101",
        attachmentId: "syllabus.txt",
      }),
    ).rejects.toThrow("disk unavailable");
  });

  it("reads the disk filename from a relative path", () => {
    expect(diskAttachmentId("materials/syllabus 1.pdf")).toBe("syllabus 1.pdf");
  });
});
