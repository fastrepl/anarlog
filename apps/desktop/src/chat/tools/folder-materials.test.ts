import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  folderAttachmentRead: vi.fn(),
  loadFolderMaterial: vi.fn(),
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    folderAttachmentRead: mocks.folderAttachmentRead,
  },
}));

vi.mock("~/session/folder-attachments", () => ({
  diskAttachmentId: (relativePath: string) => {
    const parts = relativePath.split("/");
    return parts[parts.length - 1] ?? relativePath;
  },
  loadFolderMaterial: mocks.loadFolderMaterial,
}));

import {
  buildReadFolderMaterialTool,
  folderMaterialTestInternals,
} from "./folder-materials";

describe("read_folder_material", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.loadFolderMaterial.mockResolvedValue({
      id: "mat-1",
      filename: "syllabus.txt",
      contentType: "text/plain",
      sizeBytes: 12,
      relativePath: "materials/syllabus.txt",
    });
    mocks.folderAttachmentRead.mockResolvedValue({
      status: "ok",
      data: Array.from(new TextEncoder().encode("Week 1: intro")),
    });
  });

  it("requires an active folder", async () => {
    const tool = buildReadFolderMaterialTool({} as any);
    await expect(
      (tool as any).execute({ materialId: "mat-1" }),
    ).resolves.toMatchObject({
      status: "error",
      message: "No folder is selected",
    });
    expect(mocks.loadFolderMaterial).not.toHaveBeenCalled();
  });

  it("returns readable text for folder-scoped materials", async () => {
    const tool = buildReadFolderMaterialTool({
      getFolderFilter: () => "CS 101",
    } as any);
    const result = await (tool as any).execute({ materialId: "mat-1" });

    expect(mocks.loadFolderMaterial).toHaveBeenCalledWith("CS 101", "mat-1");
    expect(mocks.folderAttachmentRead).toHaveBeenCalledWith(
      "CS 101",
      "syllabus.txt",
    );
    expect(result).toMatchObject({
      status: "ok",
      materialId: "mat-1",
      filename: "syllabus.txt",
      readable: true,
      contextText: "Week 1: intro",
    });
  });

  it("extracts text from folder-scoped PDFs", async () => {
    const { deflateSync } = await import("node:zlib");
    const compressed = deflateSync(
      Buffer.from("BT (Office hours) Tj ET", "latin1"),
    );
    const pdf = Buffer.concat([
      Buffer.from(
        `%PDF-1.1\n1 0 obj\n<< /Length ${compressed.length} /Filter /FlateDecode >>\nstream\n`,
        "latin1",
      ),
      compressed,
      Buffer.from("\nendstream\nendobj\n", "latin1"),
    ]);

    mocks.loadFolderMaterial.mockResolvedValue({
      id: "mat-2",
      filename: "syllabus.pdf",
      contentType: "application/pdf",
      sizeBytes: pdf.length,
      relativePath: "materials/syllabus.pdf",
    });
    mocks.folderAttachmentRead.mockResolvedValue({
      status: "ok",
      data: Array.from(pdf),
    });

    const tool = buildReadFolderMaterialTool({
      getFolderFilter: () => "CS 101",
    } as any);
    await expect(
      (tool as any).execute({ materialId: "mat-2" }),
    ).resolves.toMatchObject({
      status: "ok",
      filename: "syllabus.pdf",
      readable: true,
      contextText: "Office hours",
    });
    expect(
      folderMaterialTestInternals.isReadableText(
        "application/pdf",
        "syllabus.pdf",
      ),
    ).toBe(false);
  });
});
