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

export async function cleanupExpiredAudio(
  store: main.Store,
  settingsStore: settings.Store,
  nowMs = Date.now(),
) {
  const policy = normalizeAudioRetentionPolicy(
    settingsStore.getValue("audio_retention"),
  );
  const deletes: Promise<void>[] = [];

  store.forEachRow("sessions", (sessionId, _forEachCell) => {
    if (listenerStore.getState().getSessionMode(sessionId) !== "inactive") {
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
          }
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
}
