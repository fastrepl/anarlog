import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import {
  AUDIO_RETENTION_DURATION_MS,
  normalizeAudioRetention as normalizeAudioRetentionPolicy,
  type AudioRetentionPolicy,
} from "./audio-retention-policy";

import type * as main from "~/store/tinybase/store/main";
import type * as settings from "~/store/tinybase/store/settings";
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

function getAudioRetentionPolicy(settingsStore: settings.Store) {
  const hasAudioRetention = settingsStore.hasValue("audio_retention");
  const policy = normalizeAudioRetentionPolicy(
    settingsStore.getValue("audio_retention"),
  );
  const saveRecordings = settingsStore.getValue("save_recordings");

  if (!hasAudioRetention && saveRecordings === false) {
    return "none";
  }

  return policy;
}

function sessionHasTranscriptWords(store: main.Store, sessionId: string) {
  let hasWords = false;

  store.forEachRow("transcripts", (transcriptId, _forEachCell) => {
    if (hasWords) {
      return;
    }

    if (
      store.getCell("transcripts", transcriptId, "session_id") !== sessionId
    ) {
      return;
    }

    const wordsJson = store.getCell("transcripts", transcriptId, "words");
    if (typeof wordsJson !== "string" || !wordsJson) {
      return;
    }

    try {
      const words = JSON.parse(wordsJson);
      hasWords = Array.isArray(words) && words.length > 0;
    } catch {
      hasWords = false;
    }
  });

  return hasWords;
}

export async function cleanupExpiredAudio(
  store: main.Store,
  settingsStore: settings.Store,
  nowMs = Date.now(),
) {
  const policy = getAudioRetentionPolicy(settingsStore);
  const deletes: Promise<void>[] = [];
  const knownSessionIds: string[] = [];
  const deletedSessionIds: string[] = [];

  store.forEachRow("sessions", (sessionId, _forEachCell) => {
    knownSessionIds.push(sessionId);

    if (listenerStore.getState().getSessionMode(sessionId) !== "inactive") {
      return;
    }

    if (policy === "none" && !sessionHasTranscriptWords(store, sessionId)) {
      return;
    }

    const createdAt = store.getCell("sessions", sessionId, "created_at");
    if (!sessionAudioExpired(createdAt, policy, nowMs)) {
      return;
    }

    deletes.push(
      fsSyncCommands
        .audioDelete(sessionId)
        .then((result) => {
          if (result.status === "error") {
            console.error("[audio-retention] failed to delete audio", {
              sessionId,
              error: result.error,
            });
            return;
          }

          deletedSessionIds.push(sessionId);
        })
        .catch((error) => {
          console.error("[audio-retention] failed to delete audio", {
            sessionId,
            error,
          });
        }),
    );
  });

  await Promise.all(deletes);

  try {
    const orphanedResult = await fsSyncCommands.audioDeleteOrphanedExpired(
      knownSessionIds,
      AUDIO_RETENTION_DURATION_MS[policy],
      nowMs,
    );

    if (orphanedResult.status === "error") {
      console.error("[audio-retention] failed to delete orphaned audio", {
        error: orphanedResult.error,
      });
    } else {
      deletedSessionIds.push(...orphanedResult.data);
    }
  } catch (error) {
    console.error("[audio-retention] failed to delete orphaned audio", {
      error,
    });
  }

  return deletedSessionIds;
}
