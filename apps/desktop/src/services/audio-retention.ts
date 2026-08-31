import {
  AUDIO_RETENTION_DURATION_MS,
  type AudioRetentionPolicy,
} from "./audio-retention-policy";

import { liveQueryClient } from "~/db";
import {
  cleanupDeletedSessionAudio,
  deleteLocalSessionAudio,
} from "~/session/attachments";
import { listenerStore } from "~/store/zustand/listener/instance";

export const AUDIO_RETENTION_TASK_ID = "audio-retention-cleanup";
export const AUDIO_RETENTION_INTERVAL = 60 * 1000;

type SessionAudioRetentionEvent = {
  phase: "deleting" | "deleted";
  sessionId: string;
};

const sessionAudioRetentionListeners = new Set<
  (event: SessionAudioRetentionEvent) => void
>();

export function subscribeToSessionAudioRetention(
  listener: (event: SessionAudioRetentionEvent) => void,
) {
  sessionAudioRetentionListeners.add(listener);
  return () => sessionAudioRetentionListeners.delete(listener);
}

function emitSessionAudioRetention(event: SessionAudioRetentionEvent) {
  sessionAudioRetentionListeners.forEach((listener) => listener(event));
}

async function deleteWithRetentionLifecycle(
  sessionId: string,
  deleteAudio: () => Promise<boolean>,
) {
  emitSessionAudioRetention({ phase: "deleting", sessionId });
  const deleted = await deleteAudio();
  if (deleted) {
    emitSessionAudioRetention({ phase: "deleted", sessionId });
  }
  return deleted;
}

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

export function isSessionAudioIdle(sessionId: string) {
  const state = listenerStore.getState();
  return (
    state.getSessionMode(sessionId) === "inactive" &&
    !(state.live.sessionId === sessionId && state.live.loading)
  );
}

async function sessionAudioIsProcessed(sessionId: string): Promise<boolean> {
  const rows = await liveQueryClient.execute<{
    has_words: number;
    transcript_processing: number;
  }>(
    `
      SELECT
        EXISTS(
          SELECT 1
          FROM transcripts
          WHERE session_id = ?
            AND deleted_at IS NULL
            AND json_valid(words_json)
            AND json_array_length(words_json) > 0
        ) AS has_words,
        EXISTS(
          SELECT 1
          FROM session_attachments
          WHERE session_id = ?
            AND source_type = 'session_audio'
            AND source_id = 'primary'
            AND deleted_at IS NULL
            AND json_valid(metadata_json)
            AND json_extract(metadata_json, '$.transcript_status') = 'processing'
        ) AS transcript_processing
    `,
    [sessionId, sessionId],
  );
  return rows[0]?.has_words === 1 && rows[0]?.transcript_processing !== 1;
}

export async function deleteProcessedAudioForRetention(
  policy: AudioRetentionPolicy,
  sessionId: string,
) {
  if (policy !== "none") {
    return false;
  }

  if (!isSessionAudioIdle(sessionId)) {
    return false;
  }

  if (!(await sessionAudioIsProcessed(sessionId))) {
    return false;
  }

  try {
    return await deleteWithRetentionLifecycle(sessionId, () =>
      deleteLocalSessionAudio(sessionId, () => isSessionAudioIdle(sessionId)),
    );
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
  const deletedSessionIds = await cleanupLogicallyDeletedAudio();
  if (policy === "forever") {
    return deletedSessionIds;
  }

  const deletes: Promise<void>[] = [];
  const sessions = await liveQueryClient.execute<{
    id: string;
    created_at: string;
    has_words: number;
    transcript_processing: number;
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
      ) AS has_words,
      EXISTS(
        SELECT 1
        FROM session_attachments AS audio
        WHERE audio.session_id = session.id
          AND audio.source_type = 'session_audio'
          AND audio.source_id = 'primary'
          AND audio.deleted_at IS NULL
          AND json_valid(audio.metadata_json)
          AND json_extract(audio.metadata_json, '$.transcript_status') = 'processing'
      ) AS transcript_processing
    FROM sessions AS session
    WHERE session.deleted_at IS NULL
    ORDER BY session.created_at, session.id
  `);

  for (const session of sessions) {
    if (!isSessionAudioIdle(session.id)) {
      continue;
    }

    if (session.transcript_processing === 1) {
      continue;
    }

    if (policy === "none" && session.has_words !== 1) {
      continue;
    }

    if (!sessionAudioExpired(session.created_at, policy, nowMs)) {
      continue;
    }

    deletes.push(
      deleteWithRetentionLifecycle(session.id, () =>
        deleteLocalSessionAudio(session.id, () =>
          isSessionAudioIdle(session.id),
        ),
      )
        .then((deleted) => {
          if (deleted) {
            deletedSessionIds.push(session.id);
          }
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

async function cleanupLogicallyDeletedAudio() {
  const rows = await liveQueryClient.execute<{ session_id: string }>(`
    SELECT DISTINCT attachment.session_id
    FROM session_attachments AS attachment
    LEFT JOIN attachment_local_state AS local
      ON local.attachment_id = attachment.id
    WHERE attachment.source_type = 'session_audio'
      AND attachment.source_id = 'primary'
      AND attachment.deleted_at IS NOT NULL
      AND COALESCE(local.availability, 'present') != 'absent'
    ORDER BY attachment.session_id
  `);
  const deletedSessionIds: string[] = [];

  await Promise.all(
    rows.map(async ({ session_id: sessionId }) => {
      if (!isSessionAudioIdle(sessionId)) {
        return;
      }

      try {
        if (
          await deleteWithRetentionLifecycle(sessionId, () =>
            cleanupDeletedSessionAudio(sessionId, () =>
              isSessionAudioIdle(sessionId),
            ),
          )
        ) {
          deletedSessionIds.push(sessionId);
        }
      } catch (error) {
        console.error("[audio-retention] failed to finish audio deletion", {
          sessionId,
          error,
        });
      }
    }),
  );

  return deletedSessionIds;
}
