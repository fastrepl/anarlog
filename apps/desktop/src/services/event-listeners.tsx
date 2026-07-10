import { type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

import { events as notificationEvents } from "@hypr/plugin-notification";
import {
  commands as updaterCommands,
  events as updaterEvents,
} from "@hypr/plugin-updater2";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { setSettingValue } from "~/settings/queries";
import { useConfigValue, useConfigValues } from "~/shared/config";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import * as main from "~/store/tinybase/store/main";
import {
  createSession,
  getOrCreateSessionForEventId,
} from "~/store/tinybase/store/sessions";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import { parseAutoStopEndedNotificationKey } from "~/stt/auto-stop-notification";
import { parseBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";
import {
  getLiveTranscriptionConfig,
  getTranscriptionLanguages,
} from "~/stt/capabilities";

type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;

const LIVE_CAPTURE_CONFIG_DEBOUNCE_MS = 750;

function shouldAutoStartNotificationSession(
  store: MainStore,
  eventId: string | null,
  triggerAppIds: string[] | null,
): boolean {
  if (triggerAppIds && triggerAppIds.length > 0) {
    return true;
  }

  if (!eventId) {
    return true;
  }

  const startedAt = store.getRow("events", eventId)?.started_at;
  if (!startedAt) {
    return false;
  }

  const startTime = new Date(String(startedAt)).getTime();
  return !Number.isNaN(startTime) && startTime <= Date.now();
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

function getSessionParticipantHumanIds(store: MainStore, sessionId: string) {
  const seen = new Set<string>();
  const participantHumanIds: string[] = [];

  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const sid = store.getCell(
      "mapping_session_participant",
      mappingId,
      "session_id",
    );
    if (sid !== sessionId) return;

    const humanId = store.getCell(
      "mapping_session_participant",
      mappingId,
      "human_id",
    );
    if (typeof humanId !== "string" || !humanId || seen.has(humanId)) {
      return;
    }

    seen.add(humanId);
    participantHumanIds.push(humanId);
  });

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
  const store = main.UI.useStore(main.STORE_ID);
  const settingsValues = useConfigValues([
    "ai_language",
    "spoken_languages",
    "current_stt_provider",
    "current_stt_model",
  ] as const);

  if (!store) {
    return null;
  }

  const settingsSignature = JSON.stringify(settingsValues);
  return (
    <LiveCaptureConfigSyncReady
      key={settingsSignature}
      settingsValues={settingsValues}
      store={store as MainStore}
    />
  );
}

function LiveCaptureConfigSyncReady({
  store,
  settingsValues,
}: {
  store: MainStore;
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

    const pushConfig = async () => {
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

      const selfHumanId = store.getValue("user_id");
      const nextConfig = {
        session_id: live.sessionId,
        languages: liveConfig.languages,
        participant_human_ids: getSessionParticipantHumanIds(
          store,
          live.sessionId,
        ),
        self_human_id: typeof selfHumanId === "string" ? selfHumanId : null,
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

    const mainListenerIds = [
      store.addTableListener("mapping_session_participant", schedulePush),
    ];
    schedulePush();

    return () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      for (const listenerId of mainListenerIds) {
        store.delListener(listenerId);
      }
    };
  });

  return null;
}

function useUpdaterEvents() {
  const openNew = useTabs((state) => state.openNew);

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    let unlisten: UnlistenFn | null = null;

    void updaterEvents.updatedEvent
      .listen(({ payload: { previous, current } }) => {
        openNew({
          type: "changelog",
          state: { previous, current },
        });
      })
      .then((f) => {
        unlisten = f;
        updaterCommands.maybeEmitUpdated();
      });

    return () => {
      unlisten?.();
    };
  }, [openNew]);
}

function useNotificationEvents() {
  const store = main.UI.useStore(main.STORE_ID);
  const ignoredPlatforms = useConfigValue("ignored_platforms");
  const openNew = useTabs((state) => state.openNew);
  const pendingAutoStart = useRef<{
    eventId: string | null;
    triggerAppIds: string[] | null;
  } | null>(null);
  const storeRef = useLatestRef(store);
  const ignoredPlatformsRef = useLatestRef(ignoredPlatforms);
  const openNewRef = useLatestRef(openNew);

  useEffect(() => {
    if (pendingAutoStart.current && store) {
      const { eventId, triggerAppIds } = pendingAutoStart.current;
      pendingAutoStart.current = null;
      const sessionId = eventId
        ? getOrCreateSessionForEventId(store, eventId)
        : createSession(store);

      if (triggerAppIds && triggerAppIds.length > 0) {
        listenerStore.getState().setTriggerAppIds(triggerAppIds);
      }
      const autoStart = shouldAutoStartNotificationSession(
        store,
        eventId,
        triggerAppIds,
      );

      openNew({
        type: "sessions",
        id: sessionId,
        state: { view: null, autoStart: autoStart ? true : null },
      });
    }
  }, [store, openNew]);

  useEffect(() => {
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
          const currentStore = storeRef.current;
          if (sourceSessionId) {
            openNewRef.current({
              type: "sessions",
              id: sourceSessionId,
              state: { view: null, autoStart: null },
            });
            return;
          }

          if (!currentStore) {
            pendingAutoStart.current = { eventId, triggerAppIds };
            return;
          }
          const sessionId = eventId
            ? getOrCreateSessionForEventId(currentStore, eventId)
            : createSession(currentStore);

          if (triggerAppIds && triggerAppIds.length > 0) {
            listenerStore.getState().setTriggerAppIds(triggerAppIds);
          }
          const autoStart = shouldAutoStartNotificationSession(
            currentStore,
            eventId,
            triggerAppIds,
          );

          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null, autoStart: autoStart ? true : null },
          });
        } else if (payload.type === "notification_option_selected") {
          const currentStore = storeRef.current;
          if (!currentStore) return;

          const selectedIndex = payload.selected_index;
          const eventIds =
            payload.source?.type === "mic_detected"
              ? (payload.source.event_ids ?? [])
              : [];

          const sessionId =
            selectedIndex < eventIds.length
              ? getOrCreateSessionForEventId(
                  currentStore,
                  eventIds[selectedIndex],
                )
              : createSession(currentStore);

          if (payload.source?.type === "mic_detected") {
            const triggerAppIds = payload.source.app_ids ?? [];
            listenerStore
              .getState()
              .setTriggerAppIds(
                triggerAppIds.length > 0 ? triggerAppIds : null,
              );
          }

          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null, autoStart: true },
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
  }, []);
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
