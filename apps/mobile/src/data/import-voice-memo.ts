import { getDocumentAsync } from "expo-document-picker";
import { Directory, File, Paths } from "expo-file-system";

import { catalogSessionAudio } from "@/data/audio-catalog";
import { createSession, deleteSession } from "@/data/session";
import { transcribeSession } from "@/data/transcribe";
import { execute } from "@/db";
import { captureAnalytics } from "@/lib/analytics";
import { captureOperationalError } from "@/lib/error-reporting";
import { nowIso } from "@/lib/ids";

const CONTENT_TYPES: Record<string, string> = {
  m4a: "audio/mp4",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  ogg: "audio/ogg",
};

type AudioImportAsset = {
  uri: string;
  name: string;
  mimeType?: string | null;
  size?: number;
  lastModified?: number;
};

type AudioImportOptions = {
  preserveSessionOnFailure?: boolean;
  sessionId?: string;
  ownerUserId?: string;
  signal?: AbortSignal;
  trackCreated?: boolean;
};

function throwIfAborted(signal: AbortSignal | undefined): void {
  if (signal?.aborted) {
    throw signal.reason instanceof Error
      ? signal.reason
      : new Error("Audio import was cancelled");
  }
}

function splitName(name: string): { title: string; extension: string } {
  const dotIndex = name.lastIndexOf(".");
  if (dotIndex <= 0 || dotIndex === name.length - 1) {
    return { title: name, extension: "m4a" };
  }
  return {
    title: name.slice(0, dotIndex),
    extension: name.slice(dotIndex + 1).toLowerCase(),
  };
}

async function importAsset(
  asset: AudioImportAsset,
  entryPoint: "voice_memo_import" | "watch_recording",
  options?: AudioImportOptions,
): Promise<string> {
  throwIfAborted(options?.signal);
  const { title, extension } = splitName(asset.name);
  const createdAt =
    typeof asset.lastModified === "number" && asset.lastModified > 0
      ? new Date(asset.lastModified).toISOString()
      : nowIso();

  const sessionId = await createSession({
    sessionId: options?.sessionId,
    ownerUserId: options?.ownerUserId,
    title,
    createdAt,
    entryPoint,
    trackCreated: false,
  });
  throwIfAborted(options?.signal);

  let cataloged = false;
  let destination: File | null = null;
  try {
    const directory = new Directory(Paths.document, "sessions", sessionId);
    directory.create({ intermediates: true, idempotent: true });

    const filename = `audio.${extension}`;
    destination = new File(directory, filename);
    if (options?.sessionId && destination.exists) {
      destination.delete();
    }
    await new File(asset.uri).copy(destination);
    throwIfAborted(options?.signal);

    await catalogSessionAudio(sessionId, {
      filename,
      contentType:
        asset.mimeType ??
        CONTENT_TYPES[extension] ??
        "application/octet-stream",
      signal: options?.signal,
    });
    cataloged = true;
    if (!options?.signal?.aborted) {
      captureAnalytics("file_uploaded", {
        entry_point: entryPoint,
        file_type: "audio",
        content_type:
          asset.mimeType ??
          CONTENT_TYPES[extension] ??
          "application/octet-stream",
        size_bytes: asset.size ?? destination.size ?? 0,
      });
      if (options?.trackCreated ?? !options?.sessionId) {
        captureAnalytics("note_created", {
          entry_point: entryPoint,
          has_initial_title: Boolean(title),
        });
      }
    }
  } catch (error) {
    if (!cataloged) {
      if (options?.preserveSessionOnFailure) {
        try {
          if (destination?.exists) destination.delete();
        } catch (cleanupError) {
          captureOperationalError(cleanupError, {
            operation: "voice_memo_import_cleanup",
            level: "warning",
          });
        }
      } else {
        // The session exists before the audio does, so a failed copy or catalog
        // would otherwise strand an empty session on the timeline.
        await deleteSession(sessionId).catch((cleanupError) => {
          captureOperationalError(cleanupError, {
            operation: "voice_memo_import_cleanup",
            level: "warning",
          });
        });
      }
    }
    throw error;
  }

  void transcribeSession(sessionId);

  return sessionId;
}

export async function importRecordingIntoSession(
  sessionId: string,
  ownerUserId?: string,
): Promise<boolean> {
  const result = await getDocumentAsync({
    type: ["audio/*"],
    multiple: false,
    copyToCacheDirectory: true,
  });
  if (result.canceled) return false;

  await importAsset(result.assets[0], "voice_memo_import", {
    preserveSessionOnFailure: true,
    sessionId,
    ownerUserId,
  });
  return true;
}

export async function importWatchRecording(
  recording: {
    id: string;
    uri: string;
    recordedAt: string;
    accountUserId: string;
  },
  signal?: AbortSignal,
): Promise<string> {
  throwIfAborted(signal);
  const existing = (
    await execute<{
      owner_user_id: string;
      has_audio: number;
      transcript_status: string;
    }>(
      `SELECT
        owner_user_id,
        EXISTS (
          SELECT 1 FROM session_attachments
          WHERE session_id = sessions.id
            AND source_type = 'session_audio'
            AND deleted_at IS NULL
        ) AS has_audio,
        COALESCE((
          SELECT json_extract(metadata_json, '$.transcript_status')
          FROM session_attachments
          WHERE session_id = sessions.id
            AND source_type = 'session_audio'
            AND deleted_at IS NULL
          LIMIT 1
        ), '') AS transcript_status
      FROM sessions
      WHERE id = ?
        AND deleted_at IS NULL
      LIMIT 1`,
      [recording.id],
    )
  )[0];
  throwIfAborted(signal);
  if (existing) {
    if (existing.owner_user_id !== recording.accountUserId) {
      throw new Error("Watch recording belongs to a different session owner");
    }
    if (existing.has_audio === 1) {
      if (existing.transcript_status !== "complete") {
        void transcribeSession(recording.id);
      }
      return recording.id;
    }
  }

  return importAsset(
    {
      uri: recording.uri,
      name: "Watch recording.m4a",
      mimeType: "audio/mp4",
      lastModified: Date.parse(recording.recordedAt),
    },
    "watch_recording",
    {
      preserveSessionOnFailure: Boolean(existing),
      sessionId: recording.id,
      ownerUserId: recording.accountUserId,
      signal,
      trackCreated: !existing,
    },
  );
}
