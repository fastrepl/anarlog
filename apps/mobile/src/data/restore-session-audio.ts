import { getDocumentAsync } from "expo-document-picker";
import { Directory, File, FileMode, Paths } from "expo-file-system";

import { requestAttachmentBackupDownload } from "@/data/attachment-backup-client";
import type { SessionAudio } from "@/data/audio-catalog";
import {
  assertRestorableAudioMetadata,
  assertRestoredAudioMatches,
  restoredAudioRelativePath,
} from "@/data/audio-restore-model";
import { hashFileSha256 } from "@/data/file-sha256";
import { executeTransaction } from "@/db";
import { restoreAttachment } from "@/db/client";
import { nowIso } from "@/lib/ids";

export async function restoreSessionAudioFromPicker(
  sessionId: string,
  audio: SessionAudio,
): Promise<"cancelled" | "restored"> {
  assertRestorableAudioMetadata(audio);
  const result = await getDocumentAsync({
    type: ["audio/*"],
    multiple: false,
    copyToCacheDirectory: true,
  });
  if (result.canceled) return "cancelled";

  const directory = new Directory(Paths.document, "sessions", sessionId);
  directory.create({ intermediates: true, idempotent: true });
  const relativePath = restoredAudioRelativePath(audio.filename);
  const destination = new File(directory, relativePath);

  try {
    await new File(result.assets[0].uri).copy(destination, { overwrite: true });
    const verified = await hashFileSha256({
      open: () => destination.open(FileMode.ReadOnly),
    });
    assertRestoredAudioMatches(audio, verified);

    const [rowsAffected = 0] = await executeTransaction([
      {
        sql: `
          INSERT INTO attachment_local_state (
            attachment_id, session_id, relative_path, availability, updated_at
          )
          SELECT id, session_id, ?, 'present', ?
          FROM session_attachments
          WHERE id = ?
            AND session_id = ?
            AND sha256 = ?
            AND size_bytes = ?
            AND deleted_at IS NULL
          ON CONFLICT(attachment_id) DO UPDATE SET
            session_id = excluded.session_id,
            relative_path = excluded.relative_path,
            availability = excluded.availability,
            updated_at = excluded.updated_at
        `,
        params: [
          relativePath,
          nowIso(),
          audio.attachmentId,
          sessionId,
          audio.sha256,
          audio.sizeBytes,
        ],
      },
    ]);
    if (rowsAffected !== 1) {
      throw new Error(
        "The meeting recording changed while it was being restored. Try again.",
      );
    }
    return "restored";
  } catch (error) {
    try {
      if (destination.exists) destination.delete();
    } catch {
      // The unreferenced copy is harmless and can be replaced on retry.
    }
    throw error;
  }
}

export async function restoreSessionAudioFromCloud(
  sessionId: string,
  audio: SessionAudio,
  input: {
    accessToken: string;
    apiBaseUrl: string;
    supabaseUrl: string;
    signal?: AbortSignal;
  },
): Promise<void> {
  if (!audio.cloudObjectKey) {
    throw new Error("This recording has not finished syncing to the cloud.");
  }
  assertRestorableAudioMetadata(audio);
  const download = await requestAttachmentBackupDownload({
    accessToken: input.accessToken,
    apiBaseUrl: input.apiBaseUrl,
    objectKey: audio.cloudObjectKey,
    signal: input.signal,
  });
  const restored = await restoreAttachment(
    {
      sessionId,
      attachmentId: audio.attachmentId,
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
  assertRestoredAudioMatches(audio, {
    sha256: restored.sha256,
    sizeBytes: restored.sizeBytes,
  });
  if (
    restored.attachmentId !== audio.attachmentId ||
    restored.sessionId !== sessionId
  ) {
    throw new Error("The restored recording did not match this meeting.");
  }
}
