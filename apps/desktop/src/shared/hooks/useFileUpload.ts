import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback } from "react";

import {
  type AttachmentSaveResult,
  commands as fsSyncCommands,
} from "@anlg/plugin-fs-sync";

import { catalogLocalNoteAttachment, sha256Hex } from "~/session/attachments";
import { catalogLocalFolderMaterial } from "~/session/folder-attachments";

export type FileUploadResult = AttachmentSaveResult & {
  url: string;
};

const MAX_IPC_ATTACHMENT_BYTES = 4 * 1024 * 1024;

export function useFileUpload(sessionId: string) {
  return useCallback(
    async (file: File): Promise<FileUploadResult> => {
      const filename = file.name;
      const { data, sha256, sizeBytes } = await prepareIpcAttachment(file);

      const result = await fsSyncCommands.attachmentSave(
        sessionId,
        data,
        filename,
      );

      if (result.status === "error") {
        throw new Error(result.error);
      }

      const { path, attachmentId } = result.data;
      try {
        await catalogLocalNoteAttachment({
          sessionId,
          attachmentId,
          filename,
          contentType: file.type,
          sizeBytes,
          sha256,
        });
      } catch (error) {
        try {
          const cleanup = await fsSyncCommands.attachmentRemove(
            sessionId,
            attachmentId,
          );
          if (cleanup.status === "error") {
            console.error("[attachment] failed to roll back local file");
          }
        } catch {
          console.error("[attachment] failed to roll back local file");
        }
        throw error;
      }
      return { path, attachmentId, url: convertFileSrc(path) };
    },
    [sessionId],
  );
}

export function useFolderMaterialUpload(folderPath: string) {
  return useCallback(
    async (file: File): Promise<AttachmentSaveResult> => {
      const filename = file.name;
      const { data, sha256, sizeBytes } = await prepareIpcAttachment(file);

      const result = await fsSyncCommands.folderAttachmentSave(
        folderPath,
        data,
        filename,
      );

      if (result.status === "error") {
        throw new Error(result.error);
      }

      const { path, attachmentId } = result.data;
      try {
        await catalogLocalFolderMaterial({
          folderPath,
          attachmentId,
          filename,
          contentType: file.type,
          sizeBytes,
          sha256,
        });
      } catch (error) {
        try {
          const cleanup = await fsSyncCommands.folderAttachmentRemove(
            folderPath,
            attachmentId,
          );
          if (cleanup.status === "error") {
            console.error("[folder-material] failed to roll back local file");
          }
        } catch {
          console.error("[folder-material] failed to roll back local file");
        }
        throw error;
      }
      return { path, attachmentId };
    },
    [folderPath],
  );
}

function assertIpcAttachmentSize(size: number) {
  if (size > MAX_IPC_ATTACHMENT_BYTES) {
    throw new Error("Attachments must be smaller than 4 MB");
  }
}

async function prepareIpcAttachment(file: File) {
  assertIpcAttachmentSize(file.size);
  const arrayBuffer = await file.arrayBuffer();
  assertIpcAttachmentSize(arrayBuffer.byteLength);
  const sha256 = await sha256Hex(arrayBuffer);
  return {
    data: Array.from(new Uint8Array(arrayBuffer)),
    sha256,
    sizeBytes: arrayBuffer.byteLength,
  };
}
