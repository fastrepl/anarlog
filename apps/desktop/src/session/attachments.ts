import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands } from "@anlg/plugin-session";

import { enqueueSessionAudioOperation } from "./audio-operations";

import { enqueueDatabaseWrite } from "~/db/write-queue";

const SHA256_PATTERN = /^[0-9a-f]{64}$/;

export async function catalogLocalNoteAttachment(input: {
  sessionId: string;
  attachmentId: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
}): Promise<void> {
  const sessionId = requireText(input.sessionId, "session ID", 512);
  const attachmentId = requireBasename(input.attachmentId, "attachment ID");
  const filename = requireBasename(input.filename, "attachment filename");
  const contentType = requireText(
    input.contentType,
    "attachment content type",
    512,
    true,
  );
  if (!Number.isSafeInteger(input.sizeBytes) || input.sizeBytes < 0) {
    throw new Error("invalid attachment size");
  }
  if (!SHA256_PATTERN.test(input.sha256)) {
    throw new Error("invalid attachment checksum");
  }

  await enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await commands.catalogNoteAttachment({
      session_id: sessionId,
      attachment_id: attachmentId,
      filename,
      content_type: contentType,
      size_bytes: input.sizeBytes,
      sha256: input.sha256,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export async function catalogLocalSessionAudio(
  inputSessionId: string,
): Promise<void> {
  const sessionId = requireText(inputSessionId, "session ID", 512);
  await enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await fsSyncCommands.audioMetadata(sessionId);
    if (result.status === "error") {
      throw new Error(result.error);
    }
    if (!result.data) {
      throw new Error("audio_path_not_found");
    }

    const filename = requireBasename(result.data.filename, "audio filename");
    const contentType = requireText(
      result.data.contentType,
      "audio content type",
      512,
    );
    if (
      !Number.isSafeInteger(result.data.sizeBytes) ||
      result.data.sizeBytes < 0
    ) {
      throw new Error("invalid audio size");
    }
    if (!SHA256_PATTERN.test(result.data.sha256)) {
      throw new Error("invalid audio checksum");
    }

    const commandResult = await commands.catalogSessionAudio({
      session_id: sessionId,
      filename,
      content_type: contentType,
      size_bytes: result.data.sizeBytes,
      sha256: result.data.sha256,
    });
    if (commandResult.status === "error") {
      throw new Error(commandResult.error);
    }
  });
}

export async function markSessionAudioTranscriptionComplete(
  inputSessionId: string,
): Promise<void> {
  const sessionId = requireText(inputSessionId, "session ID", 512);
  await enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await commands.markSessionAudioTranscriptionComplete({
      session_id: sessionId,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export async function deleteSessionAudio(
  inputSessionId: string,
  canDelete: () => boolean,
): Promise<boolean> {
  const sessionId = requireText(inputSessionId, "session ID", 512);
  return enqueueSessionAudioOperation(sessionId, () =>
    enqueueDatabaseWrite(`session:${sessionId}`, async () => {
      if (!canDelete()) {
        return false;
      }
      await tombstoneSessionAudioMetadata(sessionId);
      await deleteSessionAudioFile(sessionId);
      return true;
    }),
  );
}

async function deleteSessionAudioFile(sessionId: string): Promise<boolean> {
  const result = await fsSyncCommands.audioDelete(sessionId);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  await markSessionAudioAvailability(sessionId, "absent");
  return result.data;
}

async function markSessionAudioAvailability(
  sessionId: string,
  availability: "present" | "absent",
): Promise<void> {
  if (availability === "absent") {
    const result = await commands.markSessionAudioAbsent({
      session_id: sessionId,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  }
}

async function tombstoneSessionAudioMetadata(sessionId: string): Promise<void> {
  const result = await commands.tombstoneSessionAudio({
    session_id: sessionId,
  });
  if (result.status === "error") {
    throw new Error(result.error);
  }
}

export async function sha256Hex(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

function requireBasename(value: unknown, label: string) {
  const basename = requireText(value, label, 1024);
  if (
    basename === "." ||
    basename === ".." ||
    basename.includes("/") ||
    basename.includes("\\") ||
    basename.includes("\0")
  ) {
    throw new Error(`invalid ${label}`);
  }
  return basename;
}

function requireText(
  value: unknown,
  label: string,
  maxLength: number,
  allowEmpty = false,
) {
  if (
    typeof value !== "string" ||
    (!allowEmpty && value.length === 0) ||
    value.length > maxLength ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/.test(value)
  ) {
    throw new Error(`invalid ${label}`);
  }
  return value;
}
