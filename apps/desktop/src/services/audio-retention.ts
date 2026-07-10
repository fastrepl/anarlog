import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import {
  AUDIO_RETENTION_DURATION_MS,
  type AudioRetentionPolicy,
} from "./audio-retention-policy";

import { liveQueryClient } from "~/db";
import { listenerStore } from "~/store/zustand/listener/instance";

export const AUDIO_RETENTION_TASK_ID = "audio-retention-cleanup";
export const AUDIO_RETENTION_INTERVAL = 60 * 1000;

export {
  normalizeAudioRetention,
  type AudioRetentionPolicy,
} from "./audio-retention-policy";

export function sessionAudioExpired(
  createdAt: unknown,
  policy: AudioRetentionPolicy,
  nowMs = Date.now(),
) {
  if (policy === "forever") {
    return false;
  }

  if (policy === "none") {
    return true;
  }

  if (typeof createdAt !== "string") {
    return false;
  }

  const createdAtMs = Date.parse(createdAt);
  if (!Number.isFinite(createdAtMs)) {
    return false;
  }

  return nowMs >= createdAtMs + AUDIO_RETENTION_DURATION_MS[policy];
}

async function sessionHasTranscriptWords(sessionId: string): Promise<boolean> {
  const rows = await liveQueryClient.execute<{ has_words: number }>(
    `
      SELECT EXISTS(
        SELECT 1
        FROM transcripts
        WHERE session_id = ?
          AND deleted_at IS NULL
          AND json_valid(words_json)
          AND json_array_length(words_json) > 0
      ) AS has_words
    `,
    [sessionId],
  );
  return rows[0]?.has_words === 1;
}

export async function deleteProcessedAudioForRetention(
  policy: AudioRetentionPolicy,
  sessionId: string,
) {
  if (policy !== "none") {
    return false;
  }

  if (listenerStore.getState().getSessionMode(sessionId) !== "inactive") {
    return false;
  }

  if (!(await sessionHasTranscriptWords(sessionId))) {
    return false;
  }

  try {
    const result = await fsSyncCommands.audioDelete(sessionId);
    if (result.status === "error") {
      console.error("[audio-retention] failed to delete audio", {
        sessionId,
        error: result.error,
      });
      return false;
    }

    return true;
  } catch (error) {
    console.error("[audio-retention] failed to delete audio", {
      sessionId,
      error,
    });
    return false;
  }
}

export async function cleanupExpiredAudio(
  policy: AudioRetentionPolicy,
  nowMs = Date.now(),
) {
  if (policy === "forever") {
    return [];
  }

  const deletes: Promise<void>[] = [];
  const deletedSessionIds: string[] = [];
  const sessions = await liveQueryClient.execute<{
    id: string;
    created_at: string;
    has_words: number;
  }>(`
    SELECT
      session.id,
      session.created_at,
      EXISTS(
        SELECT 1
        FROM transcripts AS transcript
        WHERE transcript.session_id = session.id
          AND transcript.deleted_at IS NULL
          AND json_valid(transcript.words_json)
          AND json_array_length(transcript.words_json) > 0
      ) AS has_words
    FROM sessions AS session
    WHERE session.deleted_at IS NULL
    ORDER BY session.created_at, session.id
  `);

  for (const session of sessions) {
    if (listenerStore.getState().getSessionMode(session.id) !== "inactive") {
      continue;
    }

    if (policy === "none" && session.has_words !== 1) {
      continue;
    }

    if (!sessionAudioExpired(session.created_at, policy, nowMs)) {
      continue;
    }

    deletes.push(
      fsSyncCommands
        .audioDelete(session.id)
        .then((result) => {
          if (result.status === "error") {
            console.error("[audio-retention] failed to delete audio", {
              sessionId: session.id,
              error: result.error,
            });
            return;
          }

          deletedSessionIds.push(session.id);
        })
        .catch((error) => {
          console.error("[audio-retention] failed to delete audio", {
            sessionId: session.id,
            error,
          });
        }),
    );
  }

  await Promise.all(deletes);

  return deletedSessionIds;
}
