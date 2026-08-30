import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  attachmentRemove: vi.fn(),
  attachmentSave: vi.fn(),
  catalogLocalFolderMaterial: vi.fn(),
  catalogLocalNoteAttachment: vi.fn(),
  convertFileSrc: vi.fn((path: string) => `asset:${path}`),
  folderAttachmentRemove: vi.fn(),
  folderAttachmentSave: vi.fn(),
  sha256Hex: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: mocks.convertFileSrc,
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    attachmentRemove: mocks.attachmentRemove,
    attachmentSave: mocks.attachmentSave,
    folderAttachmentRemove: mocks.folderAttachmentRemove,
    folderAttachmentSave: mocks.folderAttachmentSave,
  },
}));

vi.mock("~/session/attachments", () => ({
  catalogLocalNoteAttachment: mocks.catalogLocalNoteAttachment,
  sha256Hex: mocks.sha256Hex,
}));

vi.mock("~/session/folder-attachments", () => ({
  catalogLocalFolderMaterial: mocks.catalogLocalFolderMaterial,
}));

import { useFileUpload, useFolderMaterialUpload } from "./useFileUpload";

function uploadFile() {
  const bytes = new TextEncoder().encode("image bytes").buffer;
  return {
    name: "diagram.png",
    type: "image/png",
    size: bytes.byteLength,
    arrayBuffer: vi.fn().mockResolvedValue(bytes),
  } as unknown as File;
}

describe("useFileUpload", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.sha256Hex.mockResolvedValue("a".repeat(64));
    mocks.attachmentSave.mockResolvedValue({
      status: "ok",
      data: {
        path: "/vault/sessions/session-1/attachments/diagram 1.png",
        attachmentId: "diagram 1.png",
      },
    });
    mocks.attachmentRemove.mockResolvedValue({ status: "ok", data: null });
    mocks.folderAttachmentSave.mockResolvedValue({
      status: "ok",
      data: {
        path: "/vault/sessions/CS 101/materials/syllabus.txt",
        attachmentId: "syllabus.txt",
      },
    });
    mocks.folderAttachmentRemove.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.catalogLocalNoteAttachment.mockResolvedValue(undefined);
    mocks.catalogLocalFolderMaterial.mockResolvedValue(undefined);
  });

  it("hashes, saves, and catalogs the final physical attachment before returning", async () => {
    const file = uploadFile();
    const { result } = renderHook(() => useFileUpload("session-1"));
    let uploaded: Awaited<ReturnType<typeof result.current>> | undefined;

    await act(async () => {
      uploaded = await result.current(file);
    });

    expect(uploaded).toEqual({
      path: "/vault/sessions/session-1/attachments/diagram 1.png",
      attachmentId: "diagram 1.png",
      url: "asset:/vault/sessions/session-1/attachments/diagram 1.png",
    });
    expect(mocks.catalogLocalNoteAttachment).toHaveBeenCalledWith({
      sessionId: "session-1",
      attachmentId: "diagram 1.png",
      filename: "diagram.png",
      contentType: "image/png",
      sizeBytes: 11,
      sha256: "a".repeat(64),
    });
    expect(mocks.sha256Hex.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.attachmentSave.mock.invocationCallOrder[0]!,
    );
    expect(mocks.attachmentSave.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.catalogLocalNoteAttachment.mock.invocationCallOrder[0]!,
    );
  });

  it("removes the exact newly saved file when catalog persistence fails", async () => {
    const catalogError = new Error("catalog unavailable");
    mocks.catalogLocalNoteAttachment.mockRejectedValue(catalogError);
    const { result } = renderHook(() => useFileUpload("session-1"));

    await expect(result.current(uploadFile())).rejects.toBe(catalogError);
    expect(mocks.attachmentRemove).toHaveBeenCalledWith(
      "session-1",
      "diagram 1.png",
    );
  });

  it("does not write a file when checksum generation fails", async () => {
    mocks.sha256Hex.mockRejectedValue(new Error("checksum unavailable"));
    const { result } = renderHook(() => useFileUpload("session-1"));

    await expect(result.current(uploadFile())).rejects.toThrow(
      "checksum unavailable",
    );
    expect(mocks.attachmentSave).not.toHaveBeenCalled();
    expect(mocks.catalogLocalNoteAttachment).not.toHaveBeenCalled();
  });

  it("rejects oversized files before reading them into IPC memory", async () => {
    const file = {
      name: "recording.wav",
      type: "audio/wav",
      size: 4 * 1024 * 1024 + 1,
      arrayBuffer: vi.fn(),
    } as unknown as File;
    const { result } = renderHook(() => useFileUpload("session-1"));

    await expect(result.current(file)).rejects.toThrow(
      "Attachments must be smaller than 4 MB",
    );
    expect(file.arrayBuffer).not.toHaveBeenCalled();
    expect(mocks.attachmentSave).not.toHaveBeenCalled();
  });

  it("validates actual attachment bytes before expanding them for IPC", async () => {
    const file = {
      name: "recording.wav",
      type: "audio/wav",
      size: 0,
      arrayBuffer: vi
        .fn()
        .mockResolvedValue(new ArrayBuffer(4 * 1024 * 1024 + 1)),
    } as unknown as File;
    const { result } = renderHook(() => useFileUpload("session-1"));

    await expect(result.current(file)).rejects.toThrow(
      "Attachments must be smaller than 4 MB",
    );
    expect(mocks.sha256Hex).not.toHaveBeenCalled();
    expect(mocks.attachmentSave).not.toHaveBeenCalled();
  });

  it("saves folder materials against the folder path", async () => {
    const file = uploadFile();
    const { result } = renderHook(() => useFolderMaterialUpload("CS 101"));
    let uploaded: Awaited<ReturnType<typeof result.current>> | undefined;

    await act(async () => {
      uploaded = await result.current(file);
    });

    expect(uploaded).toEqual({
      path: "/vault/sessions/CS 101/materials/syllabus.txt",
      attachmentId: "syllabus.txt",
    });
    expect(mocks.folderAttachmentSave).toHaveBeenCalledWith(
      "CS 101",
      expect.any(Array),
      "diagram.png",
    );
    expect(mocks.catalogLocalFolderMaterial).toHaveBeenCalledWith({
      folderPath: "CS 101",
      attachmentId: "syllabus.txt",
      filename: "diagram.png",
      contentType: "image/png",
      sizeBytes: 11,
      sha256: "a".repeat(64),
    });
  });

  it("removes a folder material file when catalog persistence fails", async () => {
    const catalogError = new Error("catalog unavailable");
    mocks.catalogLocalFolderMaterial.mockRejectedValue(catalogError);
    const { result } = renderHook(() => useFolderMaterialUpload("CS 101"));

    await expect(result.current(uploadFile())).rejects.toBe(catalogError);
    expect(mocks.folderAttachmentRemove).toHaveBeenCalledWith(
      "CS 101",
      "syllabus.txt",
    );
  });
});
