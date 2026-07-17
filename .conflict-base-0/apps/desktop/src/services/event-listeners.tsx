import { type UnlistenFn } from "@tauri-apps/api/event";

import { events as notificationEvents } from "@hypr/plugin-notification";
import {
  commands as updaterCommands,
  events as updaterEvents,
} from "@hypr/plugin-updater2";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { getCalendarEventStartedAt } from "~/calendar/queries";
import { liveQueryClient } from "~/db";
import { createSession, getOrCreateSessionForEventId } from "~/session/queries";
import { setSettingValue } from "~/settings/queries";
import { useConfigValue, useConfigValues } from "~/shared/config";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import { parseAutoStopEndedNotificationKey } from "~/stt/auto-stop-notification";
import { parseBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";
import {
  getLiveTranscriptionConfig,
  getTranscriptionLanguages,
} from "~/stt/capabilities";

type CaptureIdentitySqlRow = {
  session_id: string;
  owner_user_id: string;
  human_id: string | null;
};

const CAPTURE_IDENTITY_SQL = `
  SELECT
    session.id AS session_id,
    session.owner_user_id,
    participant.human_id
  FROM sessions AS session
  LEFT JOIN session_participants AS participant
    ON participant.session_id = session.id
    AND participant.human_id <> ''
    AND participant.source <> 'excluded'
    AND participant.deleted_at IS NULL
  WHERE session.deleted_at IS NULL
  ORDER BY session.id, participant.human_id
`;

const LIVE_CAPTURE_CONFIG_DEBOUNCE_MS = 750;

async function shouldAutoStartNotificationSession(
  eventId: string | null,
  triggerAppIds: string[] | null,
): Promise<boolean> {
  if (triggerAppIds && triggerAppIds.length > 0) {
    return true;
  }

  if (!eventId) {
    return true;
  }

  const startedAt = await getCalendarEventStartedAt(eventId);
  if (!startedAt) {
    return false;
  }

  const startTime = new Date(String(startedAt)).getTime();
  return !Number.isNaN(startTime) && startTime <= Date.now();
}

async function createNotificationSession(
  eventId: string | null,
  triggerAppIds: string[] | null,
): Promise<{ sessionId: string; autoStart: boolean }> {
  const sessionId = eventId
    ? await getOrCreateSessionForEventId(eventId)
    : await createSession();

  if (triggerAppIds && triggerAppIds.length > 0) {
    listenerStore.getState().setTriggerAppIds(triggerAppIds);
  }

  return {
    sessionId,
    autoStart: await shouldAutoStartNotificationSession(eventId, triggerAppIds),
  };
}

function handleAutoStopEndedNotification(
  type: "notification_confirm" | "notification_accept" | "notification_timeout",
  key: string,
): boolean {
  const sessionId = parseAutoStopEndedNotificationKey(key);
  if (!sessionId) {
    return false;
  }

  if (type === "notification_confirm") {
    return true;
  }

  const listenerState = listenerStore.getState();
  if (
    listenerState.live.status === "active" &&
    listenerState.live.sessionId === sessionId
  ) {
    listenerState.stop();
  }

  return true;
}

function getSessionParticipantHumanIds(
  rows: CaptureIdentitySqlRow[],
  sessionId: string,
) {
  const seen = new Set<string>();
  const participantHumanIds: string[] = [];

  for (const row of rows) {
    const humanId = row.human_id;
    if (row.session_id !== sessionId || !humanId || seen.has(humanId)) {
      continue;
    }

    seen.add(humanId);
    participantHumanIds.push(humanId);
  }

  return participantHumanIds;
}

function createCaptureConfigSignature(config: {
  session_id: string;
  languages: string[];
  participant_human_ids: string[];
  self_human_id: string | null;
}) {
  return JSON.stringify(config);
}

function parseStringArray(value: unknown, fallback: string[]) {
  if (Array.isArray(value)) {
    return value.filter((item): item is string => typeof item === "string");
  }

  if (typeof value !== "string") {
    return fallback;
  }

  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : fallback;
  } catch {
    return fallback;
  }
}

function getLiveConfigLanguages(aiLanguage: string, spokenLanguages: string[]) {
  return getTranscriptionLanguages(
    aiLanguage || undefined,
    parseStringArray(spokenLanguages, []),
  );
}

function LiveCaptureConfigSync() {
  const settingsValues = useConfigValues([
    "ai_language",
    "spoken_languages",
    "current_stt_provider",
    "current_stt_model",
  ] as const);

  const settingsSignature = JSON.stringify(settingsValues);
  return (
    <LiveCaptureConfigSyncReady
      key={settingsSignature}
      settingsValues={settingsValues}
    />
  );
}

function LiveCaptureConfigSyncReady({
  settingsValues,
}: {
  settingsValues: {
    ai_language: string;
    spoken_languages: string[];
    current_stt_provider: string | undefined;
    current_stt_model: string | undefined;
  };
}) {
  useMountEffect(() => {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let lastSignature: string | null = null;
    let rows: CaptureIdentitySqlRow[] = [];
    let hasSnapshot = false;
    let cancelled = false;
    let unsubscribeDatabase: (() => Promise<void>) | null = null;

    const pushConfig = async () => {
      if (!hasSnapshot) {
        return;
      }

      const live = listenerStore.getState().live;
      if (live.status !== "active" || !live.sessionId) {
        return;
      }

      const languages = getLiveConfigLanguages(
        settingsValues.ai_language,
        settingsValues.spoken_languages,
      );
      const liveConfig = await getLiveTranscriptionConfig({
        provider: settingsValues.current_stt_provider,
        model: settingsValues.current_stt_model,
        languages,
      });

      if (liveConfig.transcriptionMode === "batch") {
        return;
      }

      const session = rows.find((row) => row.session_id === live.sessionId);
      const nextConfig = {
        session_id: live.sessionId,
        languages: liveConfig.languages,
        participant_human_ids: getSessionParticipantHumanIds(
          rows,
          live.sessionId,
        ),
        self_human_id: session?.owner_user_id || null,
      };
      const signature = createCaptureConfigSignature(nextConfig);
      if (signature === lastSignature) {
        return;
      }

      lastSignature = signature;
      await listenerStore.getState().updateCaptureConfig(nextConfig);
    };

    const schedulePush = () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }

      timeoutId = setTimeout(() => {
        void pushConfig().catch((error) => {
          console.error(
            "[listener] failed to update live capture config",
            error,
          );
        });
      }, LIVE_CAPTURE_CONFIG_DEBOUNCE_MS);
    };

    const unsubscribeListener = listenerStore.subscribe(schedulePush);
    void liveQueryClient
      .subscribe<CaptureIdentitySqlRow>(CAPTURE_IDENTITY_SQL, [], {
        onData: (nextRows) => {
          rows = nextRows;
          hasSnapshot = true;
          schedulePush();
        },
        onError: (error) => {
          console.error(
            "[listener] failed to read live capture identities",
            error,
          );
        },
      })
      .then((unsubscribe) => {
        if (cancelled) {
          void unsubscribe();
        } else {
          unsubscribeDatabase = unsubscribe;
        }
      })
      .catch((error) => {
        console.error(
          "[listener] failed to subscribe to live capture identities",
          error,
        );
      });

    return () => {
      cancelled = true;
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      unsubscribeListener();
      void unsubscribeDatabase?.();
    };
  });

  return null;
}

function useUpdaterEvents() {
  const openNew = useTabs((state) => state.openNew);
  const openNewRef = useLatestRef(openNew);

  useMountEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    let unlisten: UnlistenFn | null = null;

    void updaterEvents.updatedEvent
      .listen(({ payload: { previous, current } }) => {
        openNewRef.current({
          type: "changelog",
          state: { previous, current },
        });
      })
      .then(async (f) => {
        unlisten = f;
        await updaterCommands.maybeEmitUpdated();
      });

    return () => {
      unlisten?.();
    };
  });
}

function useNotificationEvents() {
  const ignoredPlatforms = useConfigValue("ignored_platforms");
  const openNew = useTabs((state) => state.openNew);
  const ignoredPlatformsRef = useLatestRef(ignoredPlatforms);
  const openNewRef = useLatestRef(openNew);

  useMountEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    void notificationEvents.notificationEvent
      .listen(({ payload }) => {
        if (
          payload.type === "notification_confirm" ||
          payload.type === "notification_accept" ||
          payload.type === "notification_timeout"
        ) {
          if (handleAutoStopEndedNotification(payload.type, payload.key)) {
            return;
          }

          if (payload.type === "notification_timeout") {
            return;
          }

          const eventId =
            payload.source?.type === "calendar_event"
              ? payload.source.event_id
              : payload.source?.type === "mic_detected"
                ? (payload.source.event_ids?.[0] ?? null)
                : null;
          const sourceSessionId =
            payload.source?.type === "session"
              ? payload.source.session_id
              : parseBatchCompletedNotificationKey(payload.key);
          const triggerAppIds =
            payload.source?.type === "mic_detected"
              ? (payload.source.app_ids ?? null)
              : null;
          if (sourceSessionId) {
            openNewRef.current({
              type: "sessions",
              id: sourceSessionId,
              state: { view: null, autoStart: null },
            });
            return;
          }

          void createNotificationSession(eventId, triggerAppIds)
            .then(({ sessionId, autoStart }) => {
              openNewRef.current({
                type: "sessions",
                id: sessionId,
                state: { view: null, autoStart: autoStart ? true : null },
              });
            })
            .catch((error) => {
              console.error(
                "[notification] failed to open notification session",
                error,
              );
            });
        } else if (payload.type === "notification_option_selected") {
          const selectedIndex = payload.selected_index;
          const eventIds =
            payload.source?.type === "mic_detected"
              ? (payload.source.event_ids ?? [])
              : [];

          const sessionPromise =
            selectedIndex < eventIds.length
              ? getOrCreateSessionForEventId(eventIds[selectedIndex])
              : createSession();

          if (payload.source?.type === "mic_detected") {
            const triggerAppIds = payload.source.app_ids ?? [];
            listenerStore
              .getState()
              .setTriggerAppIds(
                triggerAppIds.length > 0 ? triggerAppIds : null,
              );
          }

          void sessionPromise
            .then((sessionId) => {
              openNewRef.current({
                type: "sessions",
                id: sessionId,
                state: { view: null, autoStart: true },
              });
            })
            .catch((error) => {
              console.error(
                "[notification] failed to open selected event",
                error,
              );
            });
        } else if (payload.type === "notification_footer_action") {
          if (payload.source?.type !== "mic_detected") {
            return;
          }

          const appIds = payload.source.app_ids ?? [];
          if (appIds.length === 0) {
            return;
          }

          const ignoredPlatforms = ignoredPlatformsRef.current;
          const nextIgnoredPlatforms = [
            ...new Set([...ignoredPlatforms, ...appIds]),
          ];

          if (nextIgnoredPlatforms.length === ignoredPlatforms.length) {
            return;
          }

          void setSettingValue(
            "ignored_platforms",
            JSON.stringify(nextIgnoredPlatforms),
          ).catch((error) => {
            console.error("[notification] failed to ignore platforms", error);
          });
        }
      })
      .then((f) => {
        if (cancelled) {
          f();
        } else {
          unlisten = f;
        }
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  });
}

export function EventListeners() {
  return (
    <>
      <EventListenersInner />
      <LiveCaptureConfigSync />
    </>
  );
}

function EventListenersInner() {
  useUpdaterEvents();
  useNotificationEvents();

  return null;
}
