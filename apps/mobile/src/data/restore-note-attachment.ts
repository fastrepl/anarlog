import { Directory, File, Paths } from "expo-file-system";
import * as Sharing from "expo-sharing";

import { requestAttachmentBackupDownload } from "@/data/attachment-backup-client";
import type { NoteAttachment } from "@/data/note-attachment-catalog";
import { restoreAttachment } from "@/db/client";

export async function restoreNoteAttachmentFromCloud(
  sessionId: string,
  attachment: NoteAttachment,
  input: {
    accessToken: string;
    apiBaseUrl: string;
    supabaseUrl: string;
    signal?: AbortSignal;
  },
): Promise<void> {
  if (!attachment.cloudObjectKey) {
    throw new Error("This file has not finished syncing to the cloud.");
  }
  const download = await requestAttachmentBackupDownload({
    accessToken: input.accessToken,
    apiBaseUrl: input.apiBaseUrl,
    objectKey: attachment.cloudObjectKey,
    signal: input.signal,
  });
  const restored = await restoreAttachment(
    {
      sessionId,
      attachmentId: attachment.attachmentId,
      objectId: download.objectId,
      objectKey: download.objectKey,
      signedUrl: download.signedUrl,
      supabaseUrl: input.supabaseUrl,
      ciphertextSha256: download.ciphertextSha256,
      ciphertextSizeBytes: download.ciphertextSizeBytes,
      formatVersion: download.formatVersion,
    },
    input.signal,
  );
  if (
    restored.attachmentId !== attachment.attachmentId ||
    restored.sessionId !== sessionId ||
    restored.relativePath !== attachment.relativePath ||
    restored.sha256 !== attachment.sha256 ||
    restored.sizeBytes !== attachment.sizeBytes
  ) {
    throw new Error("The restored file did not match this note.");
  }
}

export async function shareNoteAttachment(
  uri: string,
  attachment: NoteAttachment,
): Promise<void> {
  if (!(await Sharing.isAvailableAsync())) {
    throw new Error("Sharing is not available on this device.");
  }
  const directory = new Directory(
    Paths.cache,
    "note-attachment-shares",
    attachment.attachmentId,
  );
  directory.create({ intermediates: true, idempotent: true });
  const staged = new File(directory, attachment.filename);
  if (staged.exists) staged.delete();
  await new File(uri).copy(staged);
  await Sharing.shareAsync(staged.uri, {
    dialogTitle: `Share ${attachment.filename}`,
    mimeType: attachment.contentType || "application/octet-stream",
  });
}
